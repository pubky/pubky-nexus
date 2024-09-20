use super::utils::{check_member_total_engagement_post_tag, find_post_tag};
use crate::watcher::posts::utils::{check_member_total_engagement_user_posts, find_post_counts};
use crate::watcher::utils::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use pubky_common::crypto::Keypair;
use pubky_nexus::models::notification::Notification;
use pubky_nexus::models::pubky_app::{
    traits::GenerateHashId, PubkyAppPost, PubkyAppTag, PubkyAppUser,
};
use pubky_nexus::models::tag::post::TagPost;
use pubky_nexus::models::tag::stream::Taggers;
use pubky_nexus::models::tag::traits::TagCollection;
use pubky_nexus::RedisOps;

#[tokio::test]
async fn test_homeserver_tag_post() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Create a user
    let keypair = Keypair::random();

    let user = PubkyAppUser {
        bio: Some("test_homeserver_tag_post".to_string()),
        image: None,
        links: None,
        name: "Watcher:TagPost:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Step 2: Create a post under that user
    let post = PubkyAppPost {
        content: "Watcher:TagPost:User:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
    };
    let post_id = test.create_post(&user_id, &post).await?;

    // Step 3: Add a tag to the post
    let label = "cool";

    let tag = PubkyAppTag {
        uri: format!("pubky://{}/pub/pubky.app/posts/{}", user_id, post_id),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_blob = serde_json::to_vec(&tag)?;
    let tag_url = format!("pubky://{}/pub/pubky.app/tags/{}", user_id, tag.create_id());

    // Put tag
    test.create_tag(tag_url.as_str(), tag_blob).await?;

    // GRAPH_OP
    let post_tag = find_post_tag(&user_id, &post_id, label).await.unwrap();
    assert_eq!(post_tag.label, label);
    assert_eq!(post_tag.taggers_count, 1);
    assert_eq!(post_tag.taggers[0], user_id);

    // CACHE_OP
    let cache_post_tag = TagPost::get_by_id(&user_id, Some(&post_id), None, None)
        .await
        .unwrap();

    assert_eq!(cache_post_tag.is_some(), true);
    let cache_tag_details = cache_post_tag.unwrap();
    assert_eq!(cache_tag_details.len(), 1);

    // TagPost related
    assert_eq!(cache_tag_details[0].label, label);
    // Count post tag taggers: Sorted:Posts:Tag:user_id:post_id:{label}
    assert_eq!(cache_tag_details[0].taggers_count, 1);
    // Find user as tagger in the post: Posts:Taggers:user_id:post_id
    assert_eq!(cache_tag_details[0].taggers[0], user_id);

    let post_key: [&str; 2] = [&user_id, &post_id];

    // Check if post counts updated: Post:Counts:user_id:post_id
    let post_counts = find_post_counts(&post_key).await;
    assert_eq!(post_counts.tags, 1);

    // Check if the user is related with tag: Tag:Taggers:tag_name
    let (_exist, member) = Taggers::check_set_member(&[label], &user_id).await.unwrap();
    assert!(member);

    // Check global post engagement: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement = check_member_total_engagement_user_posts(&post_key)
        .await
        .unwrap();
    assert_eq!(total_engagement.is_some(), true);
    assert_eq!(total_engagement.unwrap(), 1);

    // Check if the author user has a new notification
    // Self-tagging posts should not trigger notifications.
    // Sorted:Notification:user_id
    let notifications = Notification::get_by_id(&user_id, None, None, None, None)
        .await
        .unwrap();
    assert_eq!(
        notifications.len(),
        0,
        "Post author should have 0 notification. Self tagging."
    );

    // Tag global engagement: Sorted:Tags:Global:Post:TotalEngagement
    let total_engagement = check_member_total_engagement_post_tag(&post_key, label)
        .await
        .unwrap();
    assert_eq!(total_engagement.is_some(), true);
    assert_eq!(total_engagement.unwrap(), 1);

    // TODO: Hot tag. Uncomment when DEL is impl
    // let total_engagement = Taggers::check_sorted_set_member(&TAG_GLOBAL_HOT, &tag_label_slice).await.unwrap().unwrap();
    // assert_eq!(total_engagement, 1);

    // Check if the user is related with tag
    let (_exist, member) = Taggers::check_set_member(&[label], &user_id).await.unwrap();
    assert!(member);

    // Step 5: Delete the tag
    test.client.delete(tag_url.as_str()).await?;
    test.ensure_event_processing_complete().await?;

    // // Step 6: Verify the tag has been deleted
    // let _result_post = PostView::get_by_id(&user_id, &post_id, None, None, None)
    //     .await
    //     .unwrap()
    //     .unwrap();

    // TODO: uncomment tests when fixed redis indexing
    // assert_eq!(
    //     result_post.tags[0].taggers_count, 0,
    //     "The tag should have been deleted"
    // );

    // Cleanup user and post
    test.cleanup_post(&user_id, &post_id).await?;
    test.cleanup_user(&user_id).await?;

    Ok(())
}
