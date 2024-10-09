use super::utils::{check_member_total_engagement_post_tag, find_post_tag};
use crate::watcher::posts::utils::{check_member_total_engagement_user_posts, find_post_counts};
use crate::watcher::users::utils::find_user_counts;
use crate::watcher::utils::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use pubky_common::crypto::Keypair;
use pubky_nexus::models::notification::Notification;
use pubky_nexus::models::pubky_app::{traits::HashId, PubkyAppPost, PubkyAppTag, PubkyAppUser};
use pubky_nexus::models::tag::post::TagPost;
use pubky_nexus::models::tag::stream::Taggers;
use pubky_nexus::models::tag::traits::TagCollection;
use pubky_nexus::RedisOps;

#[tokio::test]
async fn test_homeserver_del_tag_post() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Create a user
    let keypair = Keypair::random();

    let tagger = PubkyAppUser {
        bio: Some("test_homeserver_tag_post".to_string()),
        image: None,
        links: None,
        name: "Watcher:DelTagPost:User".to_string(),
        status: None,
    };
    let tagger_user_id = test.create_user(&keypair, &tagger).await?;

    // Step 2: Create a post with a new author
    let author = PubkyAppUser {
        bio: Some("test_homeserver_tag_post".to_string()),
        image: None,
        links: None,
        name: "Watcher:DelTagPost:User".to_string(),
        status: None,
    };
    let author_user_id = test.create_user(&keypair, &author).await?;

    let post = PubkyAppPost {
        content: "Watcher:DelTagPost:User:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
    };
    let post_id = test.create_post(&author_user_id, &post).await?;

    // Step 3: Tagger user adds a tag to the his own post
    let label = "antonym";

    let tag = PubkyAppTag {
        uri: format!("pubky://{}/pub/pubky.app/posts/{}", author_user_id, post_id),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_blob = serde_json::to_vec(&tag)?;
    let tag_url = format!(
        "pubky://{}/pub/pubky.app/tags/{}",
        tagger_user_id,
        tag.create_id()
    );

    // Step 3: Creat & Delete the tag
    test.create_tag(&tag_url, tag_blob).await?;
    test.delete_tag(&tag_url).await?;

    // Step 4: Verify tag existence and data consistency
    // GRAPH_OP: Check if the tag exists in the graph database
    let post_tag = find_post_tag(&tagger_user_id, &post_id, label)
        .await
        .unwrap();
    assert!(post_tag.is_none());

    // CACHE_OP: Check if the tag is correctly cached.
    // Assert post tag indexes are updated
    // - Post:Taggers:author_id:post_id
    // - Sorted:Posts:Tag:author_id:post_id
    let cache_post_tag = TagPost::get_from_index(&author_user_id, Some(&post_id), None, None)
        .await
        .expect("Failed to get tag from cache");

    assert!(
        cache_post_tag.is_none(),
        "The SORTED SET index cannot exist for the tag"
    );

    // Check if post counts updated: Post:Counts:user_id:post_id
    let post_counts = find_post_counts(&author_user_id, &post_id).await;
    assert_eq!(post_counts.tags, 0);

    // Check if user counts updated: User:Counts:user_id
    let user_counts = find_user_counts(&author_user_id).await;
    assert_eq!(user_counts.tags, 0);

    println!(
        "tagger_user_id: {:?}, author_user_id: {:?}",
        tagger_user_id, author_user_id
    );

    // Check if the user is related with tag: Tag:Taggers:tag_name
    let (_exist, member) = Taggers::check_set_member(&[label], &tagger_user_id)
        .await
        .expect("Failed to check tagger in Taggers set");
    assert!(!member);

    let post_key: [&str; 2] = [&author_user_id, &post_id];

    // Check global post engagement: Sorted:Posts:Global:TotalEngagement:author_id:post_id
    let total_engagement = check_member_total_engagement_user_posts(&post_key)
        .await
        .expect("Failed to check total engagement for user posts");
    assert!(
        total_engagement.is_some(),
        "Total engagement should be present"
    );
    assert_eq!(total_engagement.unwrap(), 0);

    // Tag global engagement: Sorted:Tags:Global:Post:TotalEngagement
    let total_engagement = check_member_total_engagement_post_tag(&post_key, label)
        .await
        .unwrap();
    assert!(total_engagement.is_some());
    assert_eq!(total_engagement.unwrap(), 0);

    // TODO: Hot tag. Uncomment when DEL is impl
    // let total_engagement = Taggers::check_sorted_set_member(&TAG_GLOBAL_HOT, &tag_label_slice).await.unwrap().unwrap();
    // assert_eq!(total_engagement, 1);

    // Cleanup user and post
    test.cleanup_post(&tagger_user_id, &post_id).await?;
    test.cleanup_user(&tagger_user_id).await?;

    Ok(())
}
