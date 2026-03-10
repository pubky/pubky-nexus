use super::utils::check_member_post_tag_global_timeline;
use super::utils::{check_member_total_engagement_post_tag, find_post_tag};
use crate::event_processor::posts::utils::{
    check_member_total_engagement_user_posts, find_post_counts,
};
use crate::event_processor::users::utils::find_user_counts;
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::event::Event;
use nexus_common::models::notification::Notification;
use nexus_common::models::post::PostDetails;
use nexus_common::models::tag::post::TagPost;
use nexus_common::models::tag::search::TagSearch;
use nexus_common::models::tag::traits::TagCollection;
use nexus_common::types::Pagination;
use pubky::Keypair;
use pubky_app_specs::post_uri_builder;
use pubky_app_specs::{PubkyAppPost, PubkyAppTag, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_put_tag_post() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Create a user
    let user_kp = Keypair::random();

    let tagger = PubkyAppUser {
        bio: Some("test_homeserver_put_tag_post".to_string()),
        image: None,
        links: None,
        name: "Watcher:PutTagPost:User".to_string(),
        status: None,
    };
    let tagger_user_id = test.create_user(&user_kp, &tagger).await?;

    // Step 2: Create a post under that user
    let post = PubkyAppPost {
        content: "Watcher:PutTagPost:User:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    // Step 3: Tagger user adds a tag to the his own post
    let label = "merkle_tree";

    let tag = PubkyAppTag {
        uri: post_uri_builder(tagger_user_id.clone(), post_id.clone()),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();

    let (_, events_in_redis_before) = Event::get_events_from_redis(None, 1000).await.unwrap();
    // Put tag
    test.put(&user_kp, &tag_path, tag).await?;

    // Step 4: Verify tag existence and data consistency

    // GRAPH_OP: Check if the tag exists in the graph database
    let post_tag = find_post_tag(&tagger_user_id, &post_id, label)
        .await
        .unwrap()
        .expect("Failed to find post tag in graph database");

    assert_eq!(post_tag.label, label);
    assert_eq!(post_tag.taggers_count, 1);
    assert_eq!(post_tag.taggers[0], tagger_user_id);

    // CACHE_OP: Check if the tag is correctly cached
    let (_, events_in_redis_after) = Event::get_events_from_redis(None, 1000).await.unwrap();
    assert!(events_in_redis_after > events_in_redis_before);
    let cache_post_tag = TagPost::get_from_index(
        &tagger_user_id,
        Some(&post_id),
        None,
        None,
        None,
        None,
        false,
    )
    .await
    .unwrap();

    assert!(cache_post_tag.is_some());
    let cache_tag_details = cache_post_tag.unwrap();
    assert_eq!(cache_tag_details.len(), 1);

    // TagPost related
    assert_eq!(cache_tag_details[0].label, label);
    // Count post tag taggers: Sorted:Posts:Tag:user_id:post_id:{label}
    assert_eq!(cache_tag_details[0].taggers_count, 1);
    // Find user as tagger in the post: Posts:Taggers:user_id:post_id
    assert_eq!(cache_tag_details[0].taggers[0], tagger_user_id);

    let post_key: [&str; 2] = [&tagger_user_id, &post_id];

    // Assert if the new tag increments the score of engagement
    // Tag global engagement: Sorted:Tags:Global:Post:TotalEngagement
    let tag_total_engagement = check_member_total_engagement_post_tag(&post_key, label)
        .await
        .unwrap();
    assert!(tag_total_engagement.is_some());
    assert_eq!(tag_total_engagement.unwrap(), 1);

    // Assert if new tag indexes new post to the timeline
    // Tag global timeline: Sorted:Tags:Global:Post:Timeline
    let timeline = check_member_post_tag_global_timeline(&post_key, label)
        .await
        .unwrap();
    assert!(timeline.is_some());

    // Assert if the post cached time and the timeline time are the same
    let post_details = PostDetails::get_from_index(&tagger_user_id, &post_id)
        .await
        .unwrap();
    assert!(post_details.is_some());
    assert_eq!(timeline.unwrap(), post_details.unwrap().indexed_at as isize);

    // Check if post counts updated: Post:Counts:user_id:post_id
    let post_counts = find_post_counts(&tagger_user_id, &post_id).await;
    assert_eq!(post_counts.tags, 1);
    assert_eq!(post_counts.unique_tags, 1);

    // Check if user counts updated: User:Counts:user_id
    let user_counts = find_user_counts(&tagger_user_id).await;
    assert_eq!(user_counts.tagged, 1);

    // Assert if the new tag increments the engagement
    // global post engagement: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement = check_member_total_engagement_user_posts(&post_key)
        .await
        .expect("Failed to check total engagement for user posts");
    assert!(
        total_engagement.is_some(),
        "Total engagement should be present"
    );
    assert_eq!(total_engagement.unwrap(), 1);

    // Assert if the author user does not have a new notification
    // Self-tagging posts should not trigger notifications.
    // Sorted:Notification:user_id
    let notifications = Notification::get_by_id(&tagger_user_id, Pagination::default())
        .await
        .unwrap();
    assert_eq!(
        notifications.len(),
        0,
        "Post author should have 0 notification. Self tagging."
    );

    // Cleanup user and post
    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_put_tag_post_unique_count() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a user
    let tagger_kp = Keypair::random();
    let tagger = PubkyAppUser {
        bio: Some("test_homeserver_put_tag_post_unique_count".to_string()),
        image: None,
        links: None,
        name: "Watcher:PutUniqueTag:Post".to_string(),
        status: None,
    };
    let tagger_user_id = test.create_user(&tagger_kp, &tagger).await?;

    // Create a post under that user
    let post = PubkyAppPost {
        content: "Watcher:PutTagPost:User:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (post_id, post_path) = test.create_post(&tagger_kp, &post).await?;

    let label = "tag-183";
    let tag = PubkyAppTag {
        uri: post_uri_builder(tagger_user_id.clone(), post_id.clone()),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();

    // Step 1: Put tag (tag post)
    test.put(&tagger_kp, &tag_path, tag.clone()).await?;

    let post_counts_after_step_1 = find_post_counts(&tagger_user_id, &post_id).await;
    assert_eq!(post_counts_after_step_1.tags, 1);
    assert_eq!(post_counts_after_step_1.unique_tags, 1);

    // Step 2: Remove tag
    test.del(&tagger_kp, &tag_path).await?;

    let post_counts_after_step_2 = find_post_counts(&tagger_user_id, &post_id).await;
    assert_eq!(post_counts_after_step_2.tags, 0);
    assert_eq!(post_counts_after_step_2.unique_tags, 0);

    let tag_suggestions = TagSearch::get_by_label(label, &Pagination::default()).await?;
    let tag_suggestions_found = tag_suggestions.is_some_and(|x| !x.is_empty());
    assert!(!tag_suggestions_found);

    // Step 3: Re-add tag
    test.put(&tagger_kp, &tag_path, tag).await?;

    let post_counts_after_step_3 = find_post_counts(&tagger_user_id, &post_id).await;
    assert_eq!(post_counts_after_step_3.tags, 1);
    assert_eq!(post_counts_after_step_3.unique_tags, 1);

    test.cleanup_post(&tagger_kp, &post_path).await?;
    test.cleanup_user(&tagger_kp).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_put_tag_user_unique_count() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a user
    let tagger_kp = Keypair::random();
    let tagger = PubkyAppUser {
        bio: Some("test_homeserver_put_user_post_unique_count".to_string()),
        image: None,
        links: None,
        name: "Watcher:PutUniqueTag:User".to_string(),
        status: None,
    };
    let tagger_user_id = test.create_user(&tagger_kp, &tagger).await?;

    let label = "tag-237";
    let tag = PubkyAppTag {
        uri: format!("pubky://{tagger_user_id}/pub/pubky.app/profile.json"),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();

    // Step 1: Put tag (tag user)
    test.put(&tagger_kp, &tag_path, tag.clone()).await?;

    let user_counts_after_step_1 = find_user_counts(&tagger_user_id).await;
    assert_eq!(user_counts_after_step_1.tags, 1);
    assert_eq!(user_counts_after_step_1.unique_tags, 1);

    // Step 2: Remove tag
    test.del(&tagger_kp, &tag_path).await?;

    let user_counts_after_step_2 = find_user_counts(&tagger_user_id).await;
    assert_eq!(user_counts_after_step_2.tags, 0);
    assert_eq!(user_counts_after_step_2.unique_tags, 0);

    // Step 3: Re-add tag
    test.put(&tagger_kp, &tag_path, tag).await?;

    let user_counts_after_step_3 = find_user_counts(&tagger_user_id).await;
    assert_eq!(user_counts_after_step_3.tags, 1);
    assert_eq!(user_counts_after_step_3.unique_tags, 1);

    test.cleanup_user(&tagger_kp).await?;

    Ok(())
}
