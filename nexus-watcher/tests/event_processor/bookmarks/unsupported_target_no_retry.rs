use crate::event_processor::utils::watcher::{
    retrieve_and_handle_event_line, HomeserverHashIdPath, WatcherTest,
};
use anyhow::Result;
use chrono::Utc;
use nexus_watcher::events::retry::RetryScheduler;
use nexus_watcher::EventProcessorError;
use pubky::Keypair;
use pubky_social_specs::legacy_v0::{
    file_uri_builder, traits::HashId, PubkyAppBookmark, PubkyAppUser,
};

/// A bookmark of a file is rejected deterministically: only a post can be bookmarked, so the
/// event must not be queued for retry.
#[tokio_shared_rt::test(shared)]
async fn test_bookmark_file_target_is_not_retried() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_bookmark_file_target_is_not_retried".to_string()),
        image: None,
        links: None,
        name: "Watcher:Bookmark:FileTarget".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Switch OFF event processing so the bookmark is handled only by the call below
    test = test.remove_event_processing().await;

    let bookmark = PubkyAppBookmark {
        uri: file_uri_builder(user_id.clone(), "0033SSE3B1FQ0".to_string()),
        created_at: Utc::now().timestamp_millis(),
    };
    let bookmark_id = bookmark.create_id();
    test.put(&keypair, &bookmark.hs_path(), &bookmark).await?;

    let bookmark_event = format!("PUT pubky://{user_id}/pub/pubky.app/bookmarks/{bookmark_id}");
    let event_handler = test.event_processor_runner.event_handler.clone();
    let err = retrieve_and_handle_event_line(&bookmark_event, event_handler)
        .await
        .expect_err("a bookmark of a file must be rejected");

    assert!(
        err.to_string().contains("not a Post"),
        "unexpected error: {err}"
    );
    assert!(
        !RetryScheduler::should_enqueue_related_event(&err),
        "deterministic rejection must not be retried, got {err:?}"
    );
    assert!(matches!(err, EventProcessorError::UnsupportedTarget(_)));

    Ok(())
}
