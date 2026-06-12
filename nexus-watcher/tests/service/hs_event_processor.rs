use crate::service::utils::common::create_mock_handler;
use crate::service::utils::{new_in_memory_store, setup};
use anyhow::Result;
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

/// Returns a fresh random user id (z32 public key) that has no graph state yet.
fn random_user_id() -> String {
    Keypair::random().public_key().to_z32()
}

/// Creates a `User` node and, when `hs_id` is `Some`, links it to that homeserver
/// via `HOSTED_BY` (the `Homeserver` node is merged if missing).
async fn create_user_hosted_on(user_id: &str, hs_id: Option<&str>) {
    let user = UserDetails {
        id: PubkyId::try_from(user_id).expect("Valid user Pubky ID"),
        name: "test-user".to_string(),
        bio: None,
        status: None,
        links: None,
        image: None,
        indexed_at: Utc::now().timestamp_millis(),
    };
    exec_single_row(queries::put::create_user(&user).expect("create_user query"))
        .await
        .expect("create user node");

    if let Some(hs_id) = hs_id {
        exec_single_row(queries::put::set_user_homeserver(user_id, hs_id))
            .await
            .expect("link user to homeserver");
    }
}

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

// ============================================================================
// Batch continues after a single event fails
// A retryable application error on one event must not halt the batch — later
// events still need to be handed to the event handler.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_batch_continues_after_single_failure() -> Result<()> {
    setup().await?;

    // Fresh user with no HOSTED_BY edge, so the guard processes its events.
    let user_id = random_user_id();
    let first_post_id = "failone";
    let second_post_id = "failtwo";
    let first_uri = post_uri_builder(user_id.clone(), first_post_id.to_string());
    let second_uri = post_uri_builder(user_id, second_post_id.to_string());

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

    // Fresh user with no HOSTED_BY edge, so the guard processes its events.
    let post_id = "originhs";
    let uri = post_uri_builder(random_user_id(), post_id.to_string());

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
// HOSTED_BY guard: skip events from users bound to a different homeserver
// When the event's user has a HOSTED_BY edge pointing at a homeserver other
// than this processor's, the event must be skipped without reaching the handler.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_skips_event_when_user_hosted_on_different_homeserver() -> Result<()> {
    setup().await?;

    // User is bound to a homeserver that is NOT this processor's homeserver.
    let user_id = random_user_id();
    let other_hs_id = PubkyId::from(Keypair::random().public_key()).to_string();
    create_user_hosted_on(&user_id, Some(&other_hs_id)).await;

    let uri = post_uri_builder(user_id.clone(), "skipme".to_string());

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
        "Event from a user hosted on a different homeserver must be skipped"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// HOSTED_BY guard: process events from users bound to this homeserver
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_processes_event_when_user_hosted_on_same_homeserver() -> Result<()> {
    setup().await?;

    // User is bound to this processor's homeserver.
    let user_id = random_user_id();
    create_user_hosted_on(&user_id, Some(TEST_HS_ID)).await;

    let uri = post_uri_builder(user_id.clone(), "keepme".to_string());

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
        "Event from a user hosted on this homeserver must be processed"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// HOSTED_BY guard: skip events when the mapping to this homeserver is stale
// A stale edge means the user's published homeserver has diverged, so events
// must be paused until the resolver realigns the mapping.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_skips_event_when_user_mapping_to_this_homeserver_is_stale() -> Result<()> {
    setup().await?;

    // User is bound to this processor's homeserver, but the mapping is stale.
    let user_id = random_user_id();
    create_user_hosted_on(&user_id, Some(TEST_HS_ID)).await;
    exec_single_row(queries::put::set_user_homeserver_stale(&user_id, true))
        .await
        .expect("mark user homeserver mapping stale");

    let uri = post_uri_builder(user_id.clone(), "staleme".to_string());

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
        "Event from a user with a stale mapping to this homeserver must be skipped"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// HOSTED_BY guard: process events from users with no HOSTED_BY edge
// A missing edge is the common case (e.g. before the resolver has run) and must
// not block processing.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_processes_event_when_user_has_no_homeserver_edge() -> Result<()> {
    setup().await?;

    // Fresh user id with no graph state: no User node, no HOSTED_BY edge.
    let user_id = random_user_id();
    let uri = post_uri_builder(user_id, "noedge".to_string());

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
        "Event from a user without a HOSTED_BY edge must be processed"
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

    // Fresh user with no HOSTED_BY edge, so the guard processes its events.
    let user_id = random_user_id();
    let first_post_id = "infraone";
    let second_post_id = "infratwo";
    let first_uri = post_uri_builder(user_id.clone(), first_post_id.to_string());
    let second_uri = post_uri_builder(user_id, second_post_id.to_string());

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
