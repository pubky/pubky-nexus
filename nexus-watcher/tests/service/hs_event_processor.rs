use crate::service::utils::common::create_mock_handler;
use crate::service::utils::{new_in_memory_store, setup, TEST_USER_ID};
use anyhow::{Error, Result};
use chrono::Utc;
use nexus_common::db::{exec_single_row, queries};
use nexus_common::models::event::EventProcessorError;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::user::UserDetails;
use nexus_watcher::events::retry::{InitialBackoff, RetryScheduler, RetryStore};
use nexus_watcher::events::EventHandler;
use nexus_watcher::service::HsEventProcessor;
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, PubkyId};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch;

const TEST_HS_ID: &str = "1hb71xx9km3f4pw5izsy1gn19ff1uuuqonw4mcygzobwkryujoiy";

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
    let hs_id = PubkyId::try_from(TEST_HS_ID).expect("Valid test Pubky ID");

    Arc::new(HsEventProcessor {
        homeserver: Homeserver::new(hs_id),
        limit: 100,
        files_path: PathBuf::from("/tmp/test"),
        event_handler,
        shutdown_rx,
        retry_scheduler,
    })
}

async fn persist_user(user_id: &str) -> Result<()> {
    let user_id = PubkyId::try_from(user_id).map_err(Error::msg)?;
    let user = UserDetails {
        id: user_id,
        name: "hs-event-processor-test-user".into(),
        bio: None,
        status: None,
        links: None,
        image: None,
        indexed_at: Utc::now().timestamp_millis(),
    };

    exec_single_row(queries::put::create_user(&user)?).await?;
    Ok(())
}

async fn persist_user_homeserver(user_id: &str, homeserver_id: &str) -> Result<()> {
    persist_user(user_id).await?;
    exec_single_row(queries::put::set_user_homeserver(user_id, homeserver_id)).await?;
    Ok(())
}

fn random_user_id() -> String {
    Keypair::random().public_key().to_z32()
}

// ============================================================================
// Batch continues after a single event fails
// A retryable application error on one event must not halt the batch — later
// events still need to be handed to the event handler.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_batch_continues_after_single_failure() -> Result<()> {
    setup().await?;
    persist_user_homeserver(TEST_USER_ID, TEST_HS_ID).await?;

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
// Enqueued retries carry the origin homeserver id
// A retryable failure persists the processor's homeserver onto the RetryEvent.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_retry_event_carries_origin_homeserver_id() -> Result<()> {
    setup().await?;
    persist_user_homeserver(TEST_USER_ID, TEST_HS_ID).await?;

    let post_id = "originhs";
    let uri = post_uri_builder(TEST_USER_ID.to_string(), post_id.to_string());

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let handler = create_mock_handler(
        Err(EventProcessorError::Generic("handler fails".to_string())),
        None,
    );

    let processor = build_processor(store.clone(), handler.clone(), shutdown_rx);

    processor
        .process_event_lines(vec![format!("PUT {uri}")])
        .await?;

    let retry_event = store
        .get(&uri)
        .await?
        .expect("Retryable failure must enqueue a RetryEvent");
    assert_eq!(
        retry_event.origin_homeserver_id, TEST_HS_ID,
        "Enqueued retry must carry the origin homeserver id"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// Infrastructure error stops the batch
// Errors that should not be retried right now propagate out of `handle_error`,
// short-circuiting the loop so the cursor is not advanced past unprocessed events.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_batch_stops_on_infrastructure_error() -> Result<()> {
    setup().await?;
    persist_user_homeserver(TEST_USER_ID, TEST_HS_ID).await?;

    let first_post_id = "infraone";
    let second_post_id = "infratwo";
    let first_uri = post_uri_builder(TEST_USER_ID.to_string(), first_post_id.to_string());
    let second_uri = post_uri_builder(TEST_USER_ID.to_string(), second_post_id.to_string());

    let lines = vec![format!("PUT {first_uri}"), format!("PUT {second_uri}")];

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Scope the should-not-retry-now error to the first event only. The handler
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
    let processor = build_processor(store.clone(), handler.clone(), shutdown_rx);

    let result = processor.process_event_lines(lines).await;
    assert!(
        result.is_err(),
        "Should-not-retry-now error must propagate and stop the batch"
    );

    // Definitive proof: handler was called exactly once, so the batch stopped
    // after the first event and never reached the second.
    assert_eq!(
        handler.get_handle_count(),
        1,
        "Handler must be called exactly once — batch stopped on should-not-retry-now error"
    );

    // Should-not-retry-now errors bypass the retry scheduler entirely.
    assert!(
        store.get(&first_uri).await?.is_none(),
        "Should-not-retry-now errors must not be queued for retry"
    );
    assert!(
        store.get(&second_uri).await?.is_none(),
        "Second event must not be queued — batch should have stopped at the first failure"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_event_without_hosted_by_is_queued_for_retry() -> Result<()> {
    setup().await?;

    let user_id = random_user_id();
    persist_user(&user_id).await?;
    let uri = post_uri_builder(user_id, "missinghs".to_string());

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    let processor = build_processor(store.clone(), handler.clone(), shutdown_rx);

    processor
        .process_event_lines(vec![format!("PUT {uri}")])
        .await?;

    assert_eq!(
        handler.get_handle_count(),
        0,
        "Event without HOSTED_BY must not be processed immediately"
    );
    assert!(
        store.get(&uri).await?.is_some(),
        "Event without HOSTED_BY must be queued for retry"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_event_hosted_by_different_homeserver_is_skipped() -> Result<()> {
    setup().await?;

    let user_id = random_user_id();
    let other_hs_id = "8rsrmfrn1anbrzuxiffwy1174o58emf4qgbfk5h7s8a33r3bd8dy";
    persist_user_homeserver(&user_id, other_hs_id).await?;
    let uri = post_uri_builder(user_id, "otherhs".to_string());

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    let processor = build_processor(store.clone(), handler.clone(), shutdown_rx);

    processor
        .process_event_lines(vec![format!("PUT {uri}")])
        .await?;

    assert_eq!(
        handler.get_handle_count(),
        0,
        "Event hosted by a different homeserver must be skipped"
    );
    assert!(
        store.get(&uri).await?.is_none(),
        "Skipped event must not be queued for retry"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_event_hosted_by_default_homeserver_is_processed() -> Result<()> {
    setup().await?;

    let user_id = random_user_id();
    persist_user_homeserver(&user_id, TEST_HS_ID).await?;
    let uri = post_uri_builder(user_id, "defaulths".to_string());

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    let processor = build_processor(store.clone(), handler.clone(), shutdown_rx);

    processor
        .process_event_lines(vec![format!("PUT {uri}")])
        .await?;

    assert_eq!(
        handler.get_handle_count(),
        1,
        "Event hosted by the processor homeserver must be processed"
    );
    assert!(
        store.get(&uri).await?.is_none(),
        "Successfully processed event must not be queued for retry"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}
