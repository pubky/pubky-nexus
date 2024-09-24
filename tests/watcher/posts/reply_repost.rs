use super::utils::{check_member_total_engagement_user_posts, find_post_counts};
use crate::watcher::users::utils::find_user_counts;
use crate::watcher::utils::WatcherTest;
use anyhow::Result;
use pubky_common::crypto::Keypair;
use pubky_nexus::models::pubky_app::{PostEmbed, PostKind, PubkyAppPost, PubkyAppUser};

#[tokio::test]
async fn test_homeserver_reply_repost() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();

    let user = PubkyAppUser {
        bio: Some("test_homeserver_reply_repost".to_string()),
        image: None,
        links: None,
        name: "Watcher:ReplyRepost:User".to_string(),
        status: None,
    };

    let user_id = test.create_user(&keypair, &user).await?;

    // Create root Post
    let parent_post = PubkyAppPost {
        content: "Watcher:ReplyRepost:User:Post".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: None,
    };

    let parent_post_id = test.create_post(&user_id, &parent_post).await?;

    // Create reply
    let parent_uri = format!("pubky://{}/pub/pubky.app/posts/{}", user_id, parent_post_id);

    let reply = PubkyAppPost {
        content: "Watcher:ReplyRepost:User:Reply".to_string(),
        kind: PostKind::Short,
        parent: Some(parent_uri.clone()),
        embed: None,
    };

    let reply_id = test.create_post(&user_id, &reply).await?;

    // Create repost
    let post_uri = format!("pubky://{}/pub/pubky.app/posts/{}", user_id, parent_post_id);

    let repost = PubkyAppPost {
        content: "Watcher:ReplyRepost:User:Repost".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: Some(PostEmbed {
            kind: PostKind::Short,
            uri: post_uri.clone(),
        }),
    };

    test.create_post(&user_id, &repost).await?;

    // CACHE_OPs

    // Check if parent post engagement: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement = check_member_total_engagement_user_posts(&[&user_id, &parent_post_id])
        .await
        .unwrap();

    assert_eq!(total_engagement.is_some(), true);
    assert_eq!(total_engagement.unwrap(), 2);

    // Assert the parent post has changed stats. Post:Counts:user_id:post_id
    let post_count = find_post_counts(&user_id, &parent_post_id).await;

    assert_eq!(post_count.replies, 1);
    assert_eq!(post_count.reposts, 1);

    // User:Counts:user_id
    let exist_count = find_user_counts(&user_id).await;
    assert_eq!(exist_count.posts, 3);

    // TODO: Impl DEL post. Assert the reply does not exist in Nexus
    test.cleanup_post(&user_id, &reply_id).await?;

    // Cleanup
    test.cleanup_user(&user_id).await?;
    test.cleanup_post(&user_id, &parent_post_id).await?;

    Ok(())
}
