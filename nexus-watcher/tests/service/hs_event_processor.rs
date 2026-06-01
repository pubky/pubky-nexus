use crate::service::utils::common::create_mock_handler;
use crate::service::utils::{mock_resolver, new_in_memory_store, setup, TEST_USER_ID};
use anyhow::Result;
use nexus_common::db::graph::Query;
use nexus_common::db::{exec_single_row, queries};
use nexus_common::models::event::EventProcessorError;
use nexus_common::models::homeserver::Homeserver;
use nexus_watcher::events::retry::{InitialBackoff, RetryScheduler, RetryStore};
use nexus_watcher::events::EventHandler;
use nexus_watcher::service::user_hs_resolver::PkdnsHomeserverResolver;
use nexus_watcher::service::HsEventProcessor;
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, PubkyId};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch;

/// Assemble an [`HsEventProcessor`] on the given homeserver with an injected
/// PKDNS resolver. Tests bypass `poll_events` by calling `process_event_lines`
/// directly with constructed event lines.
fn build_processor_with(
    hs_id: &str,
    store: Arc<dyn RetryStore>,
    event_handler: Arc<dyn EventHandler>,
    shutdown_rx: watch::Receiver<bool>,
    user_hs_resolver: Arc<dyn PkdnsHomeserverResolver>,
) -> Arc<HsEventProcessor> {
    let retry_scheduler = Arc::new(RetryScheduler::new(
        store,
        InitialBackoff {
            missing_dep_ms: 60_000,
            transient_ms: 10_000,
        },
    ));
    let hs_id = PubkyId::try_from(hs_id).expect("Valid test Pubky ID");

    Arc::new(HsEventProcessor {
        homeserver: Homeserver::new(hs_id),
        limit: 100,
        files_path: PathBuf::from("/tmp/test"),
        event_handler,
        shutdown_rx,
        retry_scheduler,
        user_hs_resolver,
    })
}

/// Processor on a fresh random homeserver, with a PKDNS mock that resolves
/// every user back to that homeserver. Event-URI users have no `HOSTED_BY`
/// mapping, so the gate falls back to PKDNS and passes — letting these
/// batch-behaviour tests run without graph setup, with no shared identifier
/// between the user and homeserver roles.
fn build_processor(
    store: Arc<dyn RetryStore>,
    event_handler: Arc<dyn EventHandler>,
    shutdown_rx: watch::Receiver<bool>,
) -> Arc<HsEventProcessor> {
    let hs_id = random_id();
    build_processor_with(
        &hs_id,
        store,
        event_handler,
        shutdown_rx,
        mock_resolver(Some(&hs_id)),
    )
}

/// Returns a fresh random Pubky z32 id (used for both users and homeservers).
fn random_id() -> String {
    Keypair::random().public_key().to_z32()
}

/// Creates a bare `User` graph node so a `HOSTED_BY` edge can be attached to it.
async fn create_user_node(user_id: &str) -> Result<()> {
    let query = Query::new(
        "test_create_user",
        "MERGE (u:User {id: $id}) SET u.name = 'test', u.indexed_at = 0 RETURN u",
    )
    .param("id", user_id.to_string());
    exec_single_row(query).await?;
    Ok(())
}

async fn delete_user_node(user_id: &str) -> Result<()> {
    exec_single_row(queries::del::delete_user(user_id)).await?;
    Ok(())
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
        retry_event.origin_homeserver_id,
        processor.homeserver.id.to_string(),
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

// ============================================================================
// PKDNS / HOSTED_BY gate
// The default homeserver may keep emitting events for a user after that user
// re-points (or unpublishes) their `_pubky` record. `should_process_event`
// must refuse such lines, so the watcher stops indexing users no longer hosted
// here. These tests exercise each branch of that gate.
// ============================================================================

/// Drives one PUT event for `user_id` through the gate and returns how many
/// times the handler was invoked (1 = processed, 0 = refused).
async fn run_single_event(
    processor: Arc<HsEventProcessor>,
    handler: Arc<crate::utils::MockEventHandler>,
    user_id: &str,
) -> Result<usize> {
    let uri = post_uri_builder(user_id.to_string(), "gatepost".to_string());
    let result = processor
        .process_event_lines(vec![format!("PUT {uri}")])
        .await;
    assert!(result.is_ok(), "gate must skip, not error: {result:?}");
    Ok(handler.get_handle_count())
}

#[tokio_shared_rt::test(shared)]
async fn test_processes_event_when_actively_hosted_here() -> Result<()> {
    setup().await?;

    let hs_id = random_id();
    let user_id = random_id();
    create_user_node(&user_id).await?;
    // Fresh, non-stale mapping to this homeserver.
    exec_single_row(queries::put::set_user_homeserver(&user_id, &hs_id)).await?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    // Resolver must not be consulted on the graph fast path.
    let processor = build_processor_with(
        &hs_id,
        new_in_memory_store(),
        handler.clone(),
        shutdown_rx,
        mock_resolver(None),
    );

    let handled = run_single_event(processor, handler, &user_id).await?;
    assert_eq!(handled, 1, "actively hosted user must be processed");

    delete_user_node(&user_id).await?;
    let _ = shutdown_tx.send(true);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_refuses_event_when_mapping_is_stale() -> Result<()> {
    setup().await?;

    let hs_id = random_id();
    let user_id = random_id();
    create_user_node(&user_id).await?;
    exec_single_row(queries::put::set_user_homeserver(&user_id, &hs_id)).await?;
    // User switched away / unpublished: resolver marked the mapping stale.
    exec_single_row(queries::put::set_user_homeserver_stale(&user_id, true)).await?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    // Even if PKDNS still claimed this HS, a stale mapping must refuse.
    let processor = build_processor_with(
        &hs_id,
        new_in_memory_store(),
        handler.clone(),
        shutdown_rx,
        mock_resolver(Some(&hs_id)),
    );

    let handled = run_single_event(processor, handler, &user_id).await?;
    assert_eq!(handled, 0, "stale mapping must refuse the event");

    delete_user_node(&user_id).await?;
    let _ = shutdown_tx.send(true);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_refuses_event_when_bound_to_another_homeserver() -> Result<()> {
    setup().await?;

    let hs_id = random_id();
    let other_hs_id = random_id();
    let user_id = random_id();
    create_user_node(&user_id).await?;
    // Bound to a different homeserver than the one processing the event.
    exec_single_row(queries::put::set_user_homeserver(&user_id, &other_hs_id)).await?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    let processor = build_processor_with(
        &hs_id,
        new_in_memory_store(),
        handler.clone(),
        shutdown_rx,
        mock_resolver(Some(&hs_id)),
    );

    let handled = run_single_event(processor, handler, &user_id).await?;
    assert_eq!(handled, 0, "user bound elsewhere must be refused");

    delete_user_node(&user_id).await?;
    let _ = shutdown_tx.send(true);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_processes_event_via_pkdns_fallback_when_no_mapping() -> Result<()> {
    setup().await?;

    let hs_id = random_id();
    // Fresh user with no HOSTED_BY edge: gate falls back to PKDNS, which points here.
    let user_id = random_id();

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    let processor = build_processor_with(
        &hs_id,
        new_in_memory_store(),
        handler.clone(),
        shutdown_rx,
        mock_resolver(Some(&hs_id)),
    );

    let handled = run_single_event(processor, handler, &user_id).await?;
    assert_eq!(
        handled, 1,
        "unmapped user pointing here in PKDNS is processed"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_refuses_event_via_pkdns_fallback_when_points_elsewhere() -> Result<()> {
    setup().await?;

    let hs_id = random_id();
    let other_hs_id = random_id();
    // Fresh user with no HOSTED_BY edge whose PKDNS record points to another HS.
    let user_id = random_id();

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    let processor = build_processor_with(
        &hs_id,
        new_in_memory_store(),
        handler.clone(),
        shutdown_rx,
        mock_resolver(Some(&other_hs_id)),
    );

    let handled = run_single_event(processor, handler, &user_id).await?;
    assert_eq!(handled, 0, "unmapped user pointing elsewhere is refused");

    let _ = shutdown_tx.send(true);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_refuses_event_via_pkdns_fallback_when_no_record() -> Result<()> {
    setup().await?;

    let hs_id = random_id();
    // Fresh user with no HOSTED_BY edge and no published PKDNS record.
    let user_id = random_id();

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    let processor = build_processor_with(
        &hs_id,
        new_in_memory_store(),
        handler.clone(),
        shutdown_rx,
        mock_resolver(None),
    );

    let handled = run_single_event(processor, handler, &user_id).await?;
    assert_eq!(handled, 0, "unmapped user with no PKDNS record is refused");

    let _ = shutdown_tx.send(true);
    Ok(())
}
