use super::utils::check_member_total_engagement_user_posts;
use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pkarr::Keypair;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_engagement() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let alice_user_keypair = Keypair::random();

    let alice = PubkyAppUser {
        bio: Some("test_homeserver_post_engagement".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostEngagement:Alice".to_string(),
        status: None,
    };
    let alice_id = test.create_user(&alice_user_keypair, &alice).await?;

    // Alice creates a new post
    let alice_post = PubkyAppPost {
        content: "Watcher:PostEngagement:Alice:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let alice_post_id = test.create_post(&alice_id, &alice_post).await?;

    let alice_post_key: [&str; 2] = [&alice_id, &alice_post_id];

    // CACHE_OP: Assert cache index exist
    // post engagement: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement = check_member_total_engagement_user_posts(&alice_post_key)
        .await
        .unwrap();
    assert!(total_engagement.is_some());
    assert_eq!(total_engagement.unwrap(), 0);

    // Create new user
    let bob_user_keypair = Keypair::random();

    let bob_user = PubkyAppUser {
        bio: Some("test_homeserver_post_engagement".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostEngagement:Bob".to_string(),
        status: None,
    };
    let bob_id = test.create_user(&bob_user_keypair, &bob_user).await?;

    // Bob replies to popular alice post
    let parent_uri = format!("pubky://{}/pub/pubky.app/posts/{}", alice_id, alice_post_id);

    let reply = PubkyAppPost {
        content: "Watcher:PostPioneer:Bob:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(parent_uri.clone()),
        embed: None,
        attachments: None,
    };

    let _reply_id = test.create_post(&bob_id, &reply).await?;

    // Create repost of alice post
    let post_uri = format!("pubky://{}/pub/pubky.app/posts/{}", alice_id, alice_post_id);

    let repost = PubkyAppPost {
        content: "Watcher:PostPioneer:Bob:Repost".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: Some(PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: post_uri.clone(),
        }),
        attachments: None,
    };

    let _repost_id = test.create_post(&bob_id, &repost).await?;

    let total_engagement = check_member_total_engagement_user_posts(&alice_post_key)
        .await
        .unwrap();
    assert!(total_engagement.is_some());
    assert_eq!(total_engagement.unwrap(), 2);

    // // // TODO: Impl DEL post. Assert the reply does not exist in Nexus
    // test.cleanup_post(&user_id, &reply_id).await?;

    // Cleanup
    test.cleanup_user(&alice_id).await?;
    test.cleanup_user(&bob_id).await?;
    //test.cleanup_post(&user_id, &parent_post_id).await?;

    Ok(())
}
