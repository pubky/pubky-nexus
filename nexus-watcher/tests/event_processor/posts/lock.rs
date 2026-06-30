use super::utils::find_post_details;
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::notification::Notification;
use nexus_common::models::post::PostDetails;
use nexus_common::types::Pagination;
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, PubkyAppPost, PubkyAppPostKind, PubkyAppTag, PubkyAppUser,
};

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

/// A lock-only edit must refresh the cache but must NOT notify interactors,
/// while a subsequent content edit still does.
#[tokio_shared_rt::test(shared)]
async fn test_lock_only_edit_sends_no_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let author_kp = Keypair::random();
    let author = PubkyAppUser {
        bio: Some("author".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostLockNotif:Author".to_string(),
        status: None,
    };
    let author_id = test.create_user(&author_kp, &author).await?;

    // A tagger is an interactor who receives `changed_post` notifications.
    let tagger_kp = Keypair::random();
    let tagger = PubkyAppUser {
        bio: Some("tagger".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostLockNotif:Tagger".to_string(),
        status: None,
    };
    let tagger_id = test.create_user(&tagger_kp, &tagger).await?;

    let mut post = PubkyAppPost {
        content: "Lock notif test".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
        lock: None,
    };
    let (post_id, post_path) = test.create_post(&author_kp, &post).await?;

    let tag = PubkyAppTag {
        uri: post_uri_builder(author_id.clone(), post_id.clone()),
        label: "lock".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    test.put(&tagger_kp, &tag.hs_path(), &tag).await?;

    // Lock-only change (content unchanged): must not notify the tagger.
    post.lock = Some("pubky://lockserver.example/pub/pubky.app/locks/abc".to_string());
    test.put(&author_kp, &post_path, &post).await?;
    let after_lock = Notification::get_by_id(&tagger_id, Pagination::default())
        .await
        .unwrap();
    assert!(
        after_lock.is_empty(),
        "a lock-only change must not notify interactors, got {after_lock:?}"
    );

    // Content edit: the notification path still works.
    post.content = "Lock notif test (edited)".to_string();
    test.put(&author_kp, &post_path, &post).await?;
    let after_edit = Notification::get_by_id(&tagger_id, Pagination::default())
        .await
        .unwrap();
    assert_eq!(
        after_edit.len(),
        1,
        "a content edit should notify the tagger"
    );

    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&author_kp).await?;
    test.cleanup_user(&tagger_kp).await?;

    Ok(())
}
