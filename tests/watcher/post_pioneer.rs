use super::utils::WatcherTest;
use anyhow::Result;
use pubky_common::crypto::Keypair;
use pubky_nexus::{
    models::{
        pubky_app::{PostEmbed, PostKind, PubkyAppPost, PubkyAppUser},
        user::{UserStream, USER_PIONEERS_KEY_PARTS},
    },
    RedisOps,
};

#[tokio::test]
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
        kind: PostKind::Short,
        parent: None,
        embed: None,
    };

    let alice_post_id = test.create_post(&alice_id, &alice_post).await?;

    // CACHE_OP: Assert cache has not been updated. Missing followers
    // pioneers score: Sorted:Users:Pioneers
    let pioneer_score = UserStream::check_sorted_set_member(&USER_PIONEERS_KEY_PARTS, &[&alice_id])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pioneer_score, 0);

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
    let pioneer_score = UserStream::check_sorted_set_member(&USER_PIONEERS_KEY_PARTS, &[&alice_id])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pioneer_score, 1);

    // Bob replies to popular alice post
    let parent_uri = format!("pubky://{}/pub/pubky.app/posts/{}", alice_id, alice_post_id);

    let reply = PubkyAppPost {
        content: "Watcher:PostPioneer:Bob:Reply".to_string(),
        kind: PostKind::Short,
        parent: Some(parent_uri.clone()),
        embed: None,
    };

    let _reply_id = test.create_post(&bob_id, &reply).await?;

    // Create repost of alice post
    let post_uri = format!("pubky://{}/pub/pubky.app/posts/{}", alice_id, alice_post_id);

    let repost = PubkyAppPost {
        content: "Watcher:PostPioneer:Bob:Repost".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: Some(PostEmbed {
            kind: PostKind::Short,
            uri: post_uri.clone(),
        }),
    };

    test.create_post(&bob_id, &repost).await?;

    // CACHE_OP: Assert if cache has been updated
    let pioneer_score = UserStream::check_sorted_set_member(&USER_PIONEERS_KEY_PARTS, &[&bob_id])
        .await
        .unwrap()
        .unwrap();
    // Pioneer score does not update because popular user does not have any interaction
    assert_eq!(pioneer_score, 0);

    // Follow Bob
    test.create_follow(&alice_id, &bob_id).await?;

    // CACHE_OP: Assert if cache has been updated
    let pioneer_score = UserStream::check_sorted_set_member(&USER_PIONEERS_KEY_PARTS, &[&bob_id])
        .await
        .unwrap()
        .unwrap();
    // Pioneer score does not update because popular user does not have any interaction
    assert_eq!(pioneer_score, 2);

    // TODO: Impl DEL post. Assert the reply does not exist in Nexus
    // test.cleanup_post(&user_id, &reply_id).await?;

    // TODO: Cleanup
    test.cleanup_user(&alice_id).await?;
    test.cleanup_user(&bob_id).await?;
    //test.cleanup_post(&user_id, &parent_post_id).await?;

    Ok(())
}
