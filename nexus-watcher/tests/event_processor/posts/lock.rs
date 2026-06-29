use super::utils::find_post_details;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::models::post::PostDetails;
use pubky::Keypair;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind, PubkyAppUser};

/// A locked post must round-trip its `lock` URL through both the graph and the
/// cache, and an unlocked post must read back as `None` in both.
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_lock_roundtrip() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_post_lock".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostLock:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let lock_url = "pubky://lockserver.example/pub/pubky.app/locks/abc".to_string();

    let locked = PubkyAppPost {
        content: "Watcher:PostLock:Locked".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
        lock: Some(lock_url.clone()),
    };
    let (locked_id, locked_path) = test.create_post(&user_kp, &locked).await?;

    let from_graph = find_post_details(&user_id, &locked_id).await.unwrap();
    assert_eq!(from_graph.lock.as_deref(), Some(lock_url.as_str()));
    let from_cache = PostDetails::get_from_index(&user_id, &locked_id)
        .await
        .unwrap()
        .expect("locked post detail not served from cache");
    assert_eq!(from_cache.lock.as_deref(), Some(lock_url.as_str()));

    // An absent lock key must read back as None, not Some(""), in both stores.
    let unlocked = PubkyAppPost {
        content: "Watcher:PostLock:Unlocked".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
        lock: None,
    };
    let (unlocked_id, unlocked_path) = test.create_post(&user_kp, &unlocked).await?;

    let unlocked_graph = find_post_details(&user_id, &unlocked_id).await.unwrap();
    assert_eq!(unlocked_graph.lock, None);
    let unlocked_cache = PostDetails::get_from_index(&user_id, &unlocked_id)
        .await
        .unwrap()
        .expect("unlocked post detail not served from cache");
    assert_eq!(unlocked_cache.lock, None);

    test.cleanup_post(&user_kp, &locked_path).await?;
    test.cleanup_post(&user_kp, &unlocked_path).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}
