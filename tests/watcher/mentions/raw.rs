use crate::watcher::utils::WatcherTest;
use super::utils::find_post_mentions;
use anyhow::Result;
use pubky_common::crypto::Keypair;
use pubky_nexus::{models::{post::PostRelationships, pubky_app::{PostKind, PubkyAppPost, PubkyAppUser}}, RedisOps};

#[tokio::test]
async fn test_homeserver_mentions() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create first user (author)
    let author_user_keypair = Keypair::random();

    let author = PubkyAppUser {
        bio: Some("test_homeserver_mentions".to_string()),
        image: None,
        links: None,
        name: "Watcher:Mentions:Author".to_string(),
        status: None,
    };
    let author_user_id = test.create_user(&author_user_keypair, &author).await?;

    // Create second user (mention 1)
    let mentioned_user_1_keypair = Keypair::random();

    let mentioned_user_1 = PubkyAppUser {
        bio: Some("test_homeserver_mentions".to_string()),
        image: None,
        links: None,
        name: "Watcher:Mentions:MentionedUser1".to_string(),
        status: None,
    };
    let mentioned_user_1_id = test
        .create_user(&mentioned_user_1_keypair, &mentioned_user_1)
        .await?;

    // Create third user (mention 2)
    let mentioned_user_2_keypair = Keypair::random();

    let mentioned_user_2 = PubkyAppUser {
        bio: Some("test_homeserver_mentions".to_string()),
        image: None,
        links: None,
        name: "Watcher:Mentions:MentionedUser2".to_string(),
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

    // GRAPH_OP
    let post_mention_users = find_post_mentions(&author_user_id, &post_id).await;

    assert_eq!(post_mention_users.len(), 2, "Could not find all mentions in the GRAPH");
    assert!(post_mention_users.contains(&mentioned_user_1_id));
    assert!(post_mention_users.contains(&mentioned_user_2_id));

    let post_relationships = PostRelationships::try_from_index_json(&[&author_user_id, &post_id]).await.unwrap();

    assert!(post_relationships.is_some(), "Post should have relationships cached");
    let mentions = post_relationships.unwrap().mentioned.unwrap_or_default();
    assert_eq!(mentions.len(), 2, "The post should have two mentions");
    assert!(mentions.contains(&mentioned_user_1_id), "Could not find the mentioned user");
    assert!(mentions.contains(&mentioned_user_2_id), "Could not find the mentioned user");

    // Cleanup
    test.cleanup_post(&author_user_id, &post_id).await?;
    test.cleanup_user(&author_user_id).await?;
    test.cleanup_user(&mentioned_user_1_id).await?;
    test.cleanup_user(&mentioned_user_2_id).await?;

    Ok(())
}
