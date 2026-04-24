use crate::service::utils::common::create_mock_handler;
use crate::service::utils::{new_in_memory_store, setup, TEST_USER_ID};
use anyhow::Result;
use nexus_common::models::event::EventProcessorError;
use nexus_common::models::homeserver::Homeserver;
use nexus_watcher::events::retry::{InitialBackoff, RetryScheduler, RetryStore};
use nexus_watcher::events::EventHandler;
use nexus_watcher::service::HsEventProcessor;
use pubky_app_specs::{post_uri_builder, PubkyId};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch;

/// Assemble an [`HsEventProcessor`] for tests. Tests bypass `poll_events` by
/// calling `process_event_lines` directly with constructed event lines.
fn build_processor(
    store: Arc<dyn RetryStore>,
    event_handler: Arc<dyn EventHandler>,
    shutdown_rx: watch::Receiver<bool>,
) -> Arc<HsEventProcessor> {
    let retry_scheduler = Arc::new(RetryScheduler::new(
        store,
        InitialBackoff {
            missing_dep_ms: 60_000,
            transient_ms: 10_000,
        },
    ));
    let hs_id = PubkyId::try_from(TEST_USER_ID).expect("Valid test Pubky ID");

    Arc::new(HsEventProcessor {
        homeserver: Homeserver::new(hs_id),
        limit: 100,
        files_path: PathBuf::from("/tmp/test"),
        moderation: crate::utils::default_moderation_tests(),
        event_handler,
        shutdown_rx,
        retry_scheduler,
    })
}

// ============================================================================
// Batch continues after a single event fails
// A retryable application error on one event must not halt the batch — later
// events still need to be handed to the event handler.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_batch_continues_after_single_failure() -> Result<()> {
    setup().await?;

    let first_post_id = "failone";
    let second_post_id = "failtwo";
    let first_uri = post_uri_builder(TEST_USER_ID.to_string(), first_post_id.to_string());
    let second_uri = post_uri_builder(TEST_USER_ID.to_string(), second_post_id.to_string());

    let lines = vec![format!("PUT {first_uri}"), format!("PUT {second_uri}")];

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Handler returns a retryable error for every event — both events should
    // therefore be enqueued by the RetryScheduler. If the batch halted on the
    // first failure, only the first URI would be present in the store.
    let handler = create_mock_handler(
        Err(EventProcessorError::Generic("handler fails".to_string())),
        None,
    );
    let handler = Arc::new(handler);
    let processor = build_processor(store.clone(), handler.clone(), shutdown_rx);

    let result = processor.process_event_lines(lines).await;
    assert!(
        result.is_ok(),
        "Retryable application error must not stop the batch"
    );

    // Both events were processed (handler called twice), proving the batch
    // continued past the first failure.
    assert_eq!(
        handler.get_handle_count(),
        2,
        "Handler must be called for both events — batch continued past failure"
    );

    assert!(
        store.get(&first_uri).await?.is_some(),
        "First event must be queued for retry"
    );
    assert!(
        store.get(&second_uri).await?.is_some(),
        "Second event must be queued for retry — proves the batch continued past the first failure"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// Infrastructure error stops the batch
// Infrastructure errors propagate out of `handle_error`, short-circuiting the
// loop so the cursor is not advanced past unprocessed events.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_batch_stops_on_infrastructure_error() -> Result<()> {
    setup().await?;

    let first_post_id = "infraone";
    let second_post_id = "infratwo";
    let first_uri = post_uri_builder(TEST_USER_ID.to_string(), first_post_id.to_string());
    let second_uri = post_uri_builder(TEST_USER_ID.to_string(), second_post_id.to_string());

    let lines = vec![format!("PUT {first_uri}"), format!("PUT {second_uri}")];

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Scope the infrastructure error to the first event only. The handler
    // returns Ok(()) for non-matching events, so if the batch continued past
    // the first failure, the second event would succeed. The invocation
    // counter provides the definitive proof: handle_count == 1 proves the
    // handler was called exactly once (first event), and the second event
    // was never reached.
    let handler = create_mock_handler(
        Err(EventProcessorError::IndexOperationFailed(
            true,
            "simulated infra failure".to_string(),
        )),
        Some(first_post_id),
    );
    let handler = Arc::new(handler);
    let processor = build_processor(store.clone(), handler.clone(), shutdown_rx);

    let result = processor.process_event_lines(lines).await;
    assert!(
        result.is_err(),
        "Infrastructure error must propagate and stop the batch"
    );

    // Definitive proof: handler was called exactly once, so the batch stopped
    // after the first event and never reached the second.
    assert_eq!(
        handler.get_handle_count(),
        1,
        "Handler must be called exactly once — batch stopped on infrastructure error"
    );

    // Infrastructure errors bypass the retry scheduler entirely.
    assert!(
        store.get(&first_uri).await?.is_none(),
        "Infrastructure errors must not be queued for retry"
    );
    assert!(
        store.get(&second_uri).await?.is_none(),
        "Second event must not be queued — batch should have stopped at the first failure"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}
