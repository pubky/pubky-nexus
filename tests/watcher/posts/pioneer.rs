use crate::watcher::users::utils::check_member_user_pioneer;
use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppUser};
use pubky_common::crypto::Keypair;

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_pioneer() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let alice_user_keypair = Keypair::random();

    let alice_user = PubkyAppUser {
        bio: Some("test_homeserver_post_pioneer".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostPioneer:Alice".to_string(),
        status: None,
    };
    let alice_id = test.create_user(&alice_user_keypair, &alice_user).await?;

    // Alice creates a new post
    let alice_post = PubkyAppPost {
        content: "Watcher:PostPioneer:Alice:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let alice_post_id = test.create_post(&alice_id, &alice_post).await?;

    // CACHE_OP: Assert cache has not been updated. Missing followers
    // pioneers score: Sorted:Users:Pioneers
    let pioneer_score = check_member_user_pioneer(&alice_id).await.unwrap();
    assert!(pioneer_score.is_some());
    assert_eq!(pioneer_score.unwrap(), 0);

    // Create new user
    let bob_user_keypair = Keypair::random();

    let bob_user = PubkyAppUser {
        bio: Some("test_homeserver_post_pioneer".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostPioneer:Bob".to_string(),
        status: None,
    };
    let bob_id = test.create_user(&bob_user_keypair, &bob_user).await?;

    // Follow Alice
    test.create_follow(&bob_id, &alice_id).await?;

    // CACHE_OP: Assert if cache has been updated
    // pioneers score: Sorted:Users:Pioneers
    let pioneer_score = check_member_user_pioneer(&alice_id).await.unwrap();
    assert!(pioneer_score.is_some());
    assert_eq!(pioneer_score.unwrap(), 1);

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

    test.create_post(&bob_id, &repost).await?;

    // CACHE_OP: Assert if cache has been updated
    let pioneer_score = check_member_user_pioneer(&bob_id).await.unwrap();
    assert!(pioneer_score.is_some());
    // Pioneer score does not update because popular user does not have any interaction
    assert_eq!(pioneer_score.unwrap(), 0);

    // Follow Bob
    test.create_follow(&alice_id, &bob_id).await?;

    // CACHE_OP: Assert if cache has been updated
    let pioneer_score = check_member_user_pioneer(&bob_id).await.unwrap();
    assert!(pioneer_score.is_some());
    assert_eq!(pioneer_score.unwrap(), 2);

    // TODO: Impl DEL post. Assert the reply does not exist in Nexus
    // test.cleanup_post(&user_id, &reply_id).await?;

    // TODO: Cleanup
    test.cleanup_user(&alice_id).await?;
    test.cleanup_user(&bob_id).await?;
    //test.cleanup_post(&user_id, &parent_post_id).await?;

    Ok(())
}
