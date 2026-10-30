use super::utils::bookmarked_post_keys;
use crate::event_processor::utils::watcher::HomeserverHashIdPath;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::models::post::Bookmark;
use nexus_watcher::events::handlers;
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, traits::HashId, PubkyAppBookmark, PubkyAppPost, PubkyAppUser, PubkyId,
};

/// A second delete of the same bookmark matches nothing and succeeds.
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
    assert!(
        Bookmark::get_by_id(&author_id, &post_id, Some(&bookmarker_id))
            .await?
            .is_some(),
        "The bookmark has to be served before the delete"
    );

    // Delete through normal event flow
    test.del(&bookmarker_kp, &bookmark_path).await?;

    // Verify fully deleted state
    assert!(
        Bookmark::get_by_id(&author_id, &post_id, Some(&bookmarker_id))
            .await?
            .is_none()
    );

    // Replay: call sync_del again — should succeed silently (graph edge already gone)
    let bookmarker_pubky_id =
        PubkyId::try_from(bookmarker_id.as_str()).map_err(anyhow::Error::msg)?;
    let result = handlers::bookmark::sync_del(bookmarker_pubky_id, bookmark_id).await;

    assert!(
        result.is_ok(),
        "Replay after full delete should return Ok, got: {result:?}"
    );
    assert!(
        Bookmark::get_by_id(&author_id, &post_id, Some(&bookmarker_id))
            .await?
            .is_none(),
        "The point read must stay empty"
    );
    assert!(
        bookmarked_post_keys(&bookmarker_id).await?.is_empty(),
        "The stream must stay empty"
    );

    // Cleanup
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&bookmarker_kp).await?;
    test.cleanup_user(&author_kp).await?;

    Ok(())
}
