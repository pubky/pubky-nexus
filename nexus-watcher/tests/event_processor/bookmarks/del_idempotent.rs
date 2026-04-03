use super::utils::find_post_bookmark;
use crate::event_processor::utils::watcher::WatcherTest;
use crate::event_processor::{
    users::utils::find_user_counts, utils::watcher::HomeserverHashIdPath,
};
use anyhow::Result;
use nexus_common::models::event::EventProcessorError;
use nexus_common::models::post::Bookmark;
use nexus_common::models::user::UserCounts;
use nexus_watcher::events::handlers;
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, traits::HashId, PubkyAppBookmark, PubkyAppPost, PubkyAppUser, PubkyId,
};

/// Simulate a retry of sync_del after a partial failure where Redis cleanup
/// succeeded but graph deletion failed. On retry, the counter must NOT be
/// decremented again (guarded by the Redis index check).
#[tokio_shared_rt::test(shared)]
async fn test_bookmark_del_retry_no_double_decrement() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create user + post + bookmark through the normal watcher flow
    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_bookmark_del_retry_no_double_decrement".to_string()),
        image: None,
        links: None,
        name: "Watcher:Bookmark:DelRetry:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let post = PubkyAppPost {
        content: "Watcher:Bookmark:DelRetry:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    let bookmark = PubkyAppBookmark {
        uri: post_uri_builder(user_id.clone(), post_id.clone()),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_id = bookmark.create_id();
    let bookmark_path = bookmark.hs_path();

    test.put(&user_kp, &bookmark_path, bookmark).await?;

    // Verify initial state: bookmark exists in graph + Redis, count = 1
    assert!(find_post_bookmark(&user_id, &post_id, &user_id)
        .await
        .is_ok());
    assert!(Bookmark::get_from_index(&user_id, &post_id, &user_id)
        .await?
        .is_some());
    assert_eq!(find_user_counts(&user_id).await.bookmarks, 1);

    // Simulate partial completion of a previous sync_del attempt:
    // Redis cleanup succeeded (index removed + counter decremented) but graph delete failed.
    Bookmark::del_from_index(&user_id, &post_id, &user_id).await?;
    UserCounts::decrement(&user_id, "bookmarks", None).await?;

    // Verify simulated state: graph still has edge, Redis is clean, counter = 0
    assert!(find_post_bookmark(&user_id, &post_id, &user_id)
        .await
        .is_ok());
    assert!(Bookmark::get_from_index(&user_id, &post_id, &user_id)
        .await?
        .is_none());
    assert_eq!(find_user_counts(&user_id).await.bookmarks, 0);

    // Retry: call sync_del directly — should complete graph cleanup without double-decrement
    let user_pubky_id = PubkyId::try_from(user_id.as_str()).map_err(anyhow::Error::msg)?;
    handlers::bookmark::sync_del(user_pubky_id, bookmark_id).await?;

    // Verify final state: graph edge deleted, counter still 0 (not decremented again)
    assert!(find_post_bookmark(&user_id, &post_id, &user_id)
        .await
        .is_err());
    assert_eq!(find_user_counts(&user_id).await.bookmarks, 0);

    // Cleanup
    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}

/// After a fully successful delete, a replay of sync_del should return Ok
#[tokio_shared_rt::test(shared)]
async fn test_bookmark_del_replay_after_success_skips() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_bookmark_del_replay_after_success_skips".to_string()),
        image: None,
        links: None,
        name: "Watcher:Bookmark:DelReplay:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let post = PubkyAppPost {
        content: "Watcher:Bookmark:DelReplay:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    let bookmark = PubkyAppBookmark {
        uri: post_uri_builder(user_id.clone(), post_id.clone()),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_id = bookmark.create_id();
    let bookmark_path = bookmark.hs_path();

    test.put(&user_kp, &bookmark_path, bookmark).await?;

    // Delete through normal event flow
    test.del(&user_kp, &bookmark_path).await?;

    // Verify fully deleted state
    assert!(find_post_bookmark(&user_id, &post_id, &user_id)
        .await
        .is_err());
    assert_eq!(find_user_counts(&user_id).await.bookmarks, 0);

    // Replay: call sync_del again — should get SkipIndexing (graph edge gone)
    let user_pubky_id = PubkyId::try_from(user_id.as_str()).map_err(anyhow::Error::msg)?;
    let result = handlers::bookmark::sync_del(user_pubky_id, bookmark_id).await;

    assert!(
        matches!(result, Err(EventProcessorError::SkipIndexing)),
        "Replay after full delete should return SkipIndexing, got: {result:?}"
    );

    // Counter must remain 0
    assert_eq!(find_user_counts(&user_id).await.bookmarks, 0);

    // Cleanup
    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}
