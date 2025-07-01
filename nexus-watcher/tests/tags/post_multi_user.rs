use crate::utils::watcher::WatcherTest;
use crate::{
    posts::utils::{check_member_total_engagement_user_posts, find_post_counts},
    tags::utils::{
        check_member_post_tag_global_timeline, check_member_total_engagement_post_tag,
        find_post_tag,
    },
    users::utils::find_user_counts,
};
use anyhow::Result;
use chrono::Utc;
use nexus_common::{
    models::{
        notification::Notification,
        tag::{
            post::TagPost,
            traits::{TagCollection, TaggersCollection},
        },
    },
    types::Pagination,
};
use pubky::Keypair;
use pubky_app_specs::{traits::HashId, PubkyAppPost, PubkyAppTag, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_multi_user_posts_tags() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Write in the homeserver and index in nexus
    let mut user_ids = Vec::with_capacity(4);
    // Create 5 users
    for index in 0..4 {
        let keypair = Keypair::random();

        let tagger = PubkyAppUser {
            bio: Some("test_homeserver_multi_user_post".to_string()),
            image: None,
            links: None,
            name: format!("Watcher:MultiUserPost:User{index}"),
            status: None,
        };
        let user_id = test.create_user(&keypair, &tagger).await?;
        user_ids.push(user_id);
    }

    let author_id = &user_ids[0];

    let post = PubkyAppPost {
        content: "Watcher:MultiUserPost:User:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    // Create a post for the current user
    let post_id = test.create_post(author_id, &post).await?;

    let tagger_a_id = &user_ids[1];
    let tagger_b_id = &user_ids[2];
    let tagger_c_id = &user_ids[3];

    let label_water = "water";
    let label_fire = "fire";

    // Step 2: Create tags
    let mut tag_urls = Vec::with_capacity(5);
    let water_taggers = [tagger_a_id, tagger_b_id, tagger_c_id];

    for tagger_id in water_taggers {
        let tag = PubkyAppTag {
            uri: format!("pubky://{author_id}/pub/pubky.app/posts/{post_id}"),
            label: label_water.to_string(),
            created_at: Utc::now().timestamp_millis(),
        };
        let tag_url = format!(
            "pubky://{}/pub/pubky.app/tags/{}",
            tagger_id,
            tag.create_id()
        );
        // Put tag
        test.put(&tag_url, tag).await?;
        tag_urls.push(tag_url)
    }

    let fire_taggers = [tagger_b_id, tagger_c_id];

    for tagger_id in fire_taggers {
        let tag = PubkyAppTag {
            uri: format!("pubky://{author_id}/pub/pubky.app/posts/{post_id}"),
            label: label_fire.to_string(),
            created_at: Utc::now().timestamp_millis(),
        };
        let tag_url = format!(
            "pubky://{}/pub/pubky.app/tags/{}",
            tagger_id,
            tag.create_id()
        );
        // Put tag
        test.put(&tag_url, tag).await?;
        tag_urls.push(tag_url)
    }

    // Step 3: Assert all the PUT operations
    // GRAPH_OP: Check if the tag exists in the graph database
    let post_water_tag = find_post_tag(author_id, &post_id, label_water)
        .await
        .unwrap()
        .expect("Failed to find post tag in graph database");

    assert_eq!(post_water_tag.label, label_water);
    assert_eq!(post_water_tag.taggers_count, 3);
    assert!(post_water_tag.taggers.contains(tagger_a_id));
    assert!(post_water_tag.taggers.contains(tagger_b_id));
    assert!(post_water_tag.taggers.contains(tagger_c_id));

    let post_fire_tag = find_post_tag(author_id, &post_id, label_fire)
        .await
        .unwrap()
        .expect("Failed to find post tag in graph database");

    assert_eq!(post_fire_tag.label, label_fire);
    assert_eq!(post_fire_tag.taggers_count, 2);
    assert!(post_fire_tag.taggers.contains(tagger_b_id));
    assert!(post_fire_tag.taggers.contains(tagger_c_id));

    // CACHE_OP: Check if the tag is correctly cached
    let cache_post_tag = <TagPost as TagCollection>::get_from_index(
        author_id,
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
    assert_eq!(cache_tag_details.len(), 2);

    // TagPost related
    assert_eq!(cache_tag_details[0].label, label_water);
    assert_eq!(cache_tag_details[1].label, label_fire);
    // Count post tag taggers: Sorted:Posts:Tag:user_id:post_id:{label}
    assert_eq!(cache_tag_details[0].taggers_count, 3);
    assert_eq!(cache_tag_details[1].taggers_count, 2);
    // Find user as tagger in the post: Posts:Taggers:user_id:post_id
    assert!(cache_tag_details[0].taggers.contains(tagger_a_id));
    assert!(cache_tag_details[0].taggers.contains(tagger_b_id));
    assert!(cache_tag_details[0].taggers.contains(tagger_c_id));
    assert!(cache_tag_details[1].taggers.contains(tagger_b_id));
    assert!(cache_tag_details[1].taggers.contains(tagger_c_id));

    let post_key: [&str; 2] = [author_id, &post_id];

    // Assert if the new tag increments the score of engagement
    // Tag global engagement: Sorted:Tags:Global:Post:TotalEngagement
    let tag_total_engagement = check_member_total_engagement_post_tag(&post_key, label_water)
        .await
        .unwrap();
    assert!(tag_total_engagement.is_some());
    assert_eq!(tag_total_engagement.unwrap(), 3);

    let tag_total_engagement = check_member_total_engagement_post_tag(&post_key, label_fire)
        .await
        .unwrap();
    assert!(tag_total_engagement.is_some());
    assert_eq!(tag_total_engagement.unwrap(), 2);

    // Check if post counts updated: Post:Counts:user_id:post_id
    let post_counts = find_post_counts(author_id, &post_id).await;
    assert_eq!(post_counts.tags, 5);
    assert_eq!(post_counts.unique_tags, 2);

    // Check if user counts updated: User:Counts:user_id
    let tagger_a_user_counts = find_user_counts(tagger_a_id).await;
    assert_eq!(tagger_a_user_counts.tagged, 1);
    let tagger_b_user_counts = find_user_counts(tagger_b_id).await;
    assert_eq!(tagger_b_user_counts.tagged, 2);
    let tagger_c_user_counts = find_user_counts(tagger_c_id).await;
    assert_eq!(tagger_c_user_counts.tagged, 2);

    // Assert if the new tag increments the engagement
    // global post engagement: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement = check_member_total_engagement_user_posts(&post_key)
        .await
        .expect("Failed to check total engagement for user posts");
    assert!(
        total_engagement.is_some(),
        "Total engagement should be present"
    );
    assert_eq!(total_engagement.unwrap(), 5);

    // Step 4: DEL tag from homeserver
    for tag_url in tag_urls {
        test.del(&tag_url).await?;
    }

    // Step 5: Assert all the DEL operations
    // GRAPH_OP: Check if the tag exists in the graph database
    let post_tag = find_post_tag(author_id, &post_id, label_water)
        .await
        .unwrap();
    assert!(post_tag.is_none());

    let post_tag = find_post_tag(author_id, &post_id, label_fire)
        .await
        .unwrap();
    assert!(post_tag.is_none());

    // CACHE_OP: Check if the tag is correctly cached.
    // - Post:Taggers:author_id:post_id:label
    // - Sorted:Posts:Tag:author_id:post_id
    let cache_post_tag = <TagPost as TagCollection>::get_from_index(
        author_id,
        Some(&post_id),
        None,
        None,
        None,
        None,
        false,
    )
    .await
    .expect("Failed to get tag from cache");
    assert!(
        cache_post_tag.unwrap().is_empty(),
        "The SORTED SET index cannot exist for the tag"
    );

    // Post:Taggers:author_id:post_id:label
    let water_label_key = vec![author_id.as_str(), post_id.as_str(), label_water];
    let water_tag_collection =
        <TagPost as TaggersCollection>::get_from_index(water_label_key, None, None, None, None)
            .await
            .unwrap();
    assert!(water_tag_collection.is_none());

    let fire_label_key = vec![author_id.as_str(), post_id.as_str(), label_fire];
    let fire_tag_collection =
        <TagPost as TaggersCollection>::get_from_index(fire_label_key, None, None, None, None)
            .await
            .unwrap();
    assert!(fire_tag_collection.is_none());

    // Check if post counts updated: Post:Counts:user_id:post_id
    let post_counts = find_post_counts(author_id, &post_id).await;
    assert_eq!(post_counts.tags, 0);
    assert_eq!(post_counts.unique_tags, 0);

    // Check if user counts updated: User:Counts:user_id
    for tagger_id in water_taggers {
        let user_counts = find_user_counts(tagger_id).await;
        assert_eq!(user_counts.tagged, 0);
    }

    let tags = [label_water, label_fire];

    // Assert tag global engagement: Sorted:Tags:Global:Post:TotalEngagement
    for label in tags {
        let total_engagement = check_member_total_engagement_post_tag(&post_key, label)
            .await
            .unwrap();
        assert!(total_engagement.is_some());
        assert_eq!(total_engagement.unwrap(), 0);
    }

    for label in tags {
        // Assert if tag deletion clears the timeline
        // Tag timeline: Sorted:Tags:Global:Post:Timeline
        let tag_timeline = check_member_post_tag_global_timeline(&post_key, label)
            .await
            .unwrap();
        assert!(tag_timeline.is_none());
    }

    let notifications = Notification::get_by_id(author_id, Pagination::default())
        .await
        .unwrap();
    assert_eq!(
        notifications.len(),
        5,
        "Post author should have 5 notification"
    );

    Ok(())
}
