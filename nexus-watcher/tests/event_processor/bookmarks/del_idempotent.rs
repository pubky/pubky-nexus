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

    // Create post author
    let author_kp = Keypair::random();
    let author = PubkyAppUser {
        bio: Some("test_bookmark_del_retry_no_double_decrement".to_string()),
        image: None,
        links: None,
        name: "Watcher:Bookmark:DelRetry:Author".to_string(),
        status: None,
    };
    let author_id = test.create_user(&author_kp, &author).await?;

    // Create bookmark owner (different user)
    let bookmarker_kp = Keypair::random();
    let bookmarker = PubkyAppUser {
        bio: Some("test_bookmark_del_retry_no_double_decrement".to_string()),
        image: None,
        links: None,
        name: "Watcher:Bookmark:DelRetry:Bookmarker".to_string(),
        status: None,
    };
    let bookmarker_id = test.create_user(&bookmarker_kp, &bookmarker).await?;

    let post = PubkyAppPost {
        content: "Watcher:Bookmark:DelRetry:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (post_id, post_path) = test.create_post(&author_kp, &post).await?;

    let bookmark = PubkyAppBookmark {
        uri: post_uri_builder(author_id.clone(), post_id.clone()),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_id = bookmark.create_id();
    let bookmark_path = bookmark.hs_path();

    test.put(&bookmarker_kp, &bookmark_path, bookmark).await?;

    // Verify initial state: bookmark exists in graph + Redis, count = 1
    assert!(find_post_bookmark(&author_id, &post_id, &bookmarker_id)
        .await
        .is_ok());
    assert!(
        Bookmark::get_from_index(&author_id, &post_id, &bookmarker_id)
            .await?
            .is_some()
    );
    assert_eq!(find_user_counts(&bookmarker_id).await.bookmarks, 1);

    // Simulate partial completion of a previous sync_del attempt:
    // Redis cleanup succeeded (index removed + counter decremented) but graph delete failed.
    Bookmark::del_from_index(&bookmarker_id, &post_id, &author_id).await?;
    UserCounts::decrement(&bookmarker_id, "bookmarks", None).await?;

    // Verify simulated state: graph still has edge, Redis is clean, counter = 0
    assert!(find_post_bookmark(&author_id, &post_id, &bookmarker_id)
        .await
        .is_ok());
    assert!(
        Bookmark::get_from_index(&author_id, &post_id, &bookmarker_id)
            .await?
            .is_none()
    );
    assert_eq!(find_user_counts(&bookmarker_id).await.bookmarks, 0);

    // Retry: call sync_del directly — should complete graph cleanup without double-decrement
    let bookmarker_pubky_id =
        PubkyId::try_from(bookmarker_id.as_str()).map_err(anyhow::Error::msg)?;
    handlers::bookmark::sync_del(bookmarker_pubky_id, bookmark_id).await?;

    // Verify final state: graph edge deleted, counter still 0 (not decremented again)
    assert!(find_post_bookmark(&author_id, &post_id, &bookmarker_id)
        .await
        .is_err());
    assert_eq!(find_user_counts(&bookmarker_id).await.bookmarks, 0);

    // Cleanup
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&bookmarker_kp).await?;
    test.cleanup_user(&author_kp).await?;

    Ok(())
}

/// After a fully successful delete, a replay of sync_del should return Ok
#[tokio_shared_rt::test(shared)]
async fn test_bookmark_del_replay_after_success_skips() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create post author
    let author_kp = Keypair::random();
    let author = PubkyAppUser {
        bio: Some("test_bookmark_del_replay_after_success_skips".to_string()),
        image: None,
        links: None,
        name: "Watcher:Bookmark:DelReplay:Author".to_string(),
        status: None,
    };
    let author_id = test.create_user(&author_kp, &author).await?;

    // Create bookmark owner (different user)
    let bookmarker_kp = Keypair::random();
    let bookmarker = PubkyAppUser {
        bio: Some("test_bookmark_del_replay_after_success_skips".to_string()),
        image: None,
        links: None,
        name: "Watcher:Bookmark:DelReplay:Bookmarker".to_string(),
        status: None,
    };
    let bookmarker_id = test.create_user(&bookmarker_kp, &bookmarker).await?;

    let post = PubkyAppPost {
        content: "Watcher:Bookmark:DelReplay:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (post_id, post_path) = test.create_post(&author_kp, &post).await?;

    let bookmark = PubkyAppBookmark {
        uri: post_uri_builder(author_id.clone(), post_id.clone()),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_id = bookmark.create_id();
    let bookmark_path = bookmark.hs_path();

    test.put(&bookmarker_kp, &bookmark_path, bookmark).await?;

    // Delete through normal event flow
    test.del(&bookmarker_kp, &bookmark_path).await?;

    // Verify fully deleted state
    assert!(find_post_bookmark(&author_id, &post_id, &bookmarker_id)
        .await
        .is_err());
    assert_eq!(find_user_counts(&bookmarker_id).await.bookmarks, 0);

    // Replay: call sync_del again — should get SkipIndexing (graph edge gone)
    let bookmarker_pubky_id =
        PubkyId::try_from(bookmarker_id.as_str()).map_err(anyhow::Error::msg)?;
    let result = handlers::bookmark::sync_del(bookmarker_pubky_id, bookmark_id).await;

    assert!(
        matches!(result, Err(EventProcessorError::SkipIndexing)),
        "Replay after full delete should return SkipIndexing, got: {result:?}"
    );

    // Counter must remain 0
    assert_eq!(find_user_counts(&bookmarker_id).await.bookmarks, 0);

    // Cleanup
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&bookmarker_kp).await?;
    test.cleanup_user(&author_kp).await?;

    Ok(())
}
