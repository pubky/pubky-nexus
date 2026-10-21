use crate::event_processor::utils::watcher::{
    retrieve_and_handle_event_line, HomeserverHashIdPath, WatcherTest,
};
use anyhow::Result;
use chrono::Utc;
use nexus_watcher::events::retry::RetryScheduler;
use nexus_watcher::EventProcessorError;
use pubky::Keypair;
use pubky_app_specs::{traits::HashId, ParsedUri, PubkyAppTag, PubkyAppUser};

/// A pubky.app tag whose target is not a pubky:// URI (e.g. an external https URL)
/// can never be parsed, so it must be rejected without being queued for retry.
#[tokio_shared_rt::test(shared)]
async fn test_tag_external_uri_is_not_retried() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_tag_external_uri_is_not_retried".to_string()),
        image: None,
        links: None,
        name: "Watcher:Tag:ExternalUri".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Switch OFF event processing so the tag is handled only by the call below
    test = test.remove_event_processing().await;

    let tag = PubkyAppTag {
        uri: "https://example.com/album/123".to_string(),
        label: "external_uri".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    assert!(ParsedUri::try_from(tag.uri.as_str()).is_err());
    let tag_id = tag.create_id();
    test.put(&keypair, &tag.hs_path(), &tag).await?;

    let tag_event = format!("PUT pubky://{user_id}/pub/pubky.app/tags/{tag_id}");
    let event_handler = test.event_processor_runner.event_handler.clone();
    let err = retrieve_and_handle_event_line(&tag_event, event_handler)
        .await
        .expect_err("tag with a non-pubky target must be rejected");

    assert!(
        !RetryScheduler::should_enqueue_related_event(&err),
        "deterministic rejection must not be retried, got {err:?}"
    );
    assert!(
        matches!(err, EventProcessorError::UnsupportedTarget(_)),
        "unexpected error: {err:?}"
    );

    Ok(())
}
