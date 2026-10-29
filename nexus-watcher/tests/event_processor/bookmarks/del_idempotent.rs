use super::utils::find_post_bookmark;
use crate::event_processor::utils::watcher::HomeserverHashIdPath;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::models::post::Bookmark;
use nexus_watcher::events::handlers;
use pubky::Keypair;
use pubky_social_specs::legacy_v0::{
    post_uri_builder, traits::HashId, PubkyAppBookmark, PubkyAppPost, PubkyAppUser,
};
use pubky_social_specs::PubkyId;

/// Simulate a retry of sync_del after a partial failure where Redis cleanup
/// succeeded but graph deletion failed. The retry must still delete the edge.
#[tokio_shared_rt::test(shared)]
async fn test_bookmark_del_retry_completes_graph_delete() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // Create post author
    let author_kp = Keypair::random();
    let author = PubkyAppUser {
        bio: Some("test_bookmark_del_retry_completes_graph_delete".to_string()),
        image: None,
        links: None,
        name: "Watcher:Bookmark:DelRetry:Author".to_string(),
        status: None,
    };
    let author_id = test.create_user(&author_kp, &author).await?;

    // Create bookmark owner (different user)
    let bookmarker_kp = Keypair::random();
    let bookmarker = PubkyAppUser {
        bio: Some("test_bookmark_del_retry_completes_graph_delete".to_string()),
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
        lock: None,
    };
    let (post_id, post_path) = test.create_post(&author_kp, &post).await?;

    let bookmark = PubkyAppBookmark {
        uri: post_uri_builder(author_id.clone(), post_id.clone()),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_id = bookmark.create_id();
    let bookmark_path = bookmark.hs_path();

    test.put(&bookmarker_kp, &bookmark_path, bookmark).await?;

    // Verify initial state: bookmark exists in graph + Redis
    assert!(find_post_bookmark(&author_id, &post_id, &bookmarker_id)
        .await
        .is_ok());
    assert!(
        Bookmark::get_from_index(&author_id, &post_id, &bookmarker_id)
            .await?
            .is_some()
    );

    // Simulate partial completion of a previous sync_del attempt:
    // Redis cleanup succeeded (index removed) but graph delete failed.
    Bookmark::del_from_index(&bookmarker_id, &post_id, &author_id).await?;

    // Verify simulated state: graph still has edge, Redis is clean
    assert!(find_post_bookmark(&author_id, &post_id, &bookmarker_id)
        .await
        .is_ok());
    assert!(
        Bookmark::get_from_index(&author_id, &post_id, &bookmarker_id)
            .await?
            .is_none()
    );

    // Retry: call sync_del directly — should complete graph cleanup
    let bookmarker_pubky_id =
        PubkyId::try_from(bookmarker_id.as_str()).map_err(anyhow::Error::msg)?;
    handlers::bookmark::sync_del(bookmarker_pubky_id, bookmark_id).await?;

    // Verify final state: graph edge deleted
    assert!(find_post_bookmark(&author_id, &post_id, &bookmarker_id)
        .await
        .is_err());

    // Cleanup
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&bookmarker_kp).await?;
    test.cleanup_user(&author_kp).await?;

    Ok(())
}

/// After a fully successful delete, a replay of sync_del should return Ok
#[tokio_shared_rt::test(shared)]
async fn test_bookmark_del_replay_after_success_skips() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

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
        lock: None,
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

    // Replay: call sync_del again — should succeed silently (graph edge already gone)
    let bookmarker_pubky_id =
        PubkyId::try_from(bookmarker_id.as_str()).map_err(anyhow::Error::msg)?;
    let result = handlers::bookmark::sync_del(bookmarker_pubky_id, bookmark_id).await;

    assert!(
        result.is_ok(),
        "Replay after full delete should return Ok, got: {result:?}"
    );

    // Cleanup
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&bookmarker_kp).await?;
    test.cleanup_user(&author_kp).await?;

    Ok(())
}
