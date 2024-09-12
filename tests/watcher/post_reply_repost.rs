use super::utils::WatcherTest;
use anyhow::Result;
use pkarr::Keypair;
use pubky_nexus::{models::{
    post::{PostCounts, PostStream, POST_TOTAL_ENGAGEMENT_KEY_PARTS},
    pubky_app::{PostEmbed, PostKind, PubkyAppPost, PubkyAppUser}, user::UserCounts,
}, RedisOps};

#[tokio::test]
async fn test_homeserver_reply_repost() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();

    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Test replyer".to_string(),
        status: None,
    };

    let user_id = test.create_user(&keypair, &user).await?;

    // Create root Post
    let parent_post = PubkyAppPost {
        content: "This is a test post!".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: None,
    };

    let parent_post_id = test.create_post(&user_id, &parent_post).await?;

    // Create reply
    let parent_uri = format!("pubky://{}/pub/pubky.app/posts/{}", user_id, parent_post_id);

    let reply = PubkyAppPost {
        content: "This is a reply post!".to_string(),
        kind: PostKind::Short,
        parent: Some(parent_uri.clone()),
        embed: None,
    };

    let reply_id = test.create_post(&user_id, &reply).await?;

    // Create repost
    let post_uri = format!("pubky://{}/pub/pubky.app/posts/{}", user_id, parent_post_id);

    let repost = PubkyAppPost {
        content: "This is a repost post!".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: Some(PostEmbed {
            kind: PostKind::Short,
            uri: post_uri.clone(),
        }),
    };

    test.create_post(&user_id, &repost).await?;

    // Check if the interaction is cached
    let parent_post_key: [&str; 2] = [&user_id, &parent_post_id];

    // Check if parent post engagement: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement =
        PostStream::check_sorted_set_member(&POST_TOTAL_ENGAGEMENT_KEY_PARTS, &parent_post_key)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(total_engagement, 2);
    
    // Assert the parent post has changed stats. Post:Counts:user_id:post_id
    let post_count = PostCounts::try_from_index_json(&parent_post_key)
        .await
        .unwrap()
        .expect("The new post was not served from Nexus");

    assert_eq!(post_count.replies, 1);
    assert_eq!(post_count.reposts, 1);

    let exist_count = UserCounts::try_from_index_json(&[&user_id]).await.unwrap()
        .expect("User count not found");

    assert_eq!(exist_count.posts, 3);

    // // TODO: Impl DEL post. Assert the reply does not exist in Nexus
    test.cleanup_post(&user_id, &reply_id).await?;

    // Cleanup
    test.cleanup_user(&user_id).await?;
    test.cleanup_post(&user_id, &parent_post_id).await?;

    Ok(())
}
