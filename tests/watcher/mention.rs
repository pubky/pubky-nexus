use super::utils::WatcherTest;
use anyhow::Result;
use pubky_common::crypto::Keypair;
use pubky_nexus::models::{
    post::PostView,
    pubky_app::{PostKind, PubkyAppPost, PubkyAppUser},
};

#[tokio::test]
async fn test_homeserver_mentions() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create first user (author)
    let author_user_keypair = Keypair::random();
    let author = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Post author".to_string(),
        status: None,
    };
    let author_user_id = test.create_user(&author_user_keypair, &author).await?;

    // Create second user (mention 1)
    let mentioned_user_1_keypair = Keypair::random();
    let mentioned_user_1 = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "First mentioned user".to_string(),
        status: None,
    };
    let mentioned_user_1_id = test
        .create_user(&mentioned_user_1_keypair, &mentioned_user_1)
        .await?;

    // Create third user (mention 2)
    let mentioned_user_2_keypair = Keypair::random();
    let mentioned_user_2 = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Second mentioned user".to_string(),
        status: None,
    };
    let mentioned_user_2_id = test
        .create_user(&mentioned_user_2_keypair, &mentioned_user_2)
        .await?;

    // User 1 writes a post mentioning User 2 and User 3
    let post_content = format!(
        "This is a post mentioning pk:{}, and also pk:{}",
        mentioned_user_1_id, mentioned_user_2_id
    );
    let post = PubkyAppPost {
        content: post_content.clone(),
        kind: PostKind::Short,
        parent: None,
        embed: None,
    };

    let post_id = test.create_post(&author_user_id, &post).await?;

    // Assert the post can be served from Nexus
    let result_post = PostView::get_by_id(&author_user_id, &post_id, None, None, None)
        .await
        .unwrap()
        .expect("The post was not served from Nexus");

    // Verify the mentioned relationships contain both User 2 and User 3
    assert!(result_post
        .relationships
        .mentioned
        .clone()
        .unwrap_or_default()
        .contains(&mentioned_user_1_id));
    assert!(result_post
        .relationships
        .mentioned
        .clone()
        .unwrap_or_default()
        .contains(&mentioned_user_2_id));
    assert_eq!(
        result_post
            .relationships
            .mentioned
            .unwrap_or_default()
            .len(),
        2
    );

    // Cleanup
    test.cleanup_post(&author_user_id, &post_id).await?;
    test.cleanup_user(&author_user_id).await?;
    test.cleanup_user(&mentioned_user_1_id).await?;
    test.cleanup_user(&mentioned_user_2_id).await?;

    Ok(())
}
