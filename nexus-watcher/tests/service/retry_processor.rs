use crate::service::utils::setup;
use crate::utils::MockEventHandler;
use anyhow::Result;
use chrono::Utc;
use nexus_common::config::EventRetryConfig;
use nexus_common::db::kv::RedisOps;
use nexus_common::models::event::{EventProcessorError, EventType};
use nexus_watcher::events::retry::{
    InMemoryRetryStore, RedisRetryStore, RetryEvent, RetryEventIndexKey, RetryProcessor, RetryStore,
};
use nexus_watcher::events::EventHandler;
use nexus_watcher::events::Moderation;
use nexus_watcher::service::TEventProcessor;
use pubky_app_specs::{post_uri_builder, PubkyId};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch;

/// Test user ID - valid 52-character z32 Pubky ID
const TEST_USER_ID: &str = "uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko";

/// Test helper to create an EventRetryConfig with custom values
fn create_test_config(
    max_retries: u32,
    max_dependency_retries: u32,
    initial_backoff_secs: u64,
    max_backoff_secs: u64,
    initial_missing_dep_backoff_secs: u64,
    max_missing_dep_backoff_secs: u64,
) -> EventRetryConfig {
    EventRetryConfig {
        max_retries,
        max_dependency_retries,
        initial_backoff_secs,
        max_backoff_secs,
        initial_missing_dep_backoff_secs,
        max_missing_dep_backoff_secs,
    }
}

/// Test helper to create a mock event handler scoped to a specific URI substring.
///
/// `target_substring` is typically the unique post_id prefix used by the test. Events
/// whose URI matches get `result`; any other events (leftover entries from parallel
/// tests in the shared retry queue) get `Ok(())` and are drained normally.
fn create_mock_handler(
    result: Result<(), EventProcessorError>,
    target_substring: &str,
) -> Arc<dyn EventHandler> {
    // Use real Moderation with a moderator ID that won't match test users
    let moderation = Arc::new(Moderation {
        id: PubkyId::try_from(TEST_USER_ID).expect("Valid test moderation key"),
        tags: vec![],
    });
    Arc::new(MockEventHandler {
        result,
        target_uri_substring: Some(target_substring.to_string()),
        moderation,
    })
}

/// Test helper to create a test RetryEvent with a valid URI
fn create_test_retry_event(
    post_id: &str,
    event_type: EventType,
    retry_count: u32,
    next_retry_at: i64,
) -> RetryEvent {
    let event_uri = post_uri_builder(TEST_USER_ID.to_string(), post_id.to_string());
    RetryEvent {
        retry_count,
        event_type,
        event_uri,
        next_retry_at,
    }
}

/// Test helper to create a resource key for a test event, matching the format the scheduler uses.
fn create_resource_key(post_id: &str) -> RetryEventIndexKey {
    post_uri_builder(TEST_USER_ID.to_string(), post_id.to_string())
}

/// Build a fresh `Arc<dyn RetryStore>` backed by in-memory state.
///
/// Each test gets its own store, so parallel test runs cannot observe or
/// mutate each other's retry events — no shared Redis queue, no pollution.
fn new_in_memory_store() -> Arc<dyn RetryStore> {
    Arc::new(InMemoryRetryStore::new())
}

/// Assemble a [`RetryProcessor`] for tests with the given store, config, and handler.
fn build_processor(
    store: Arc<dyn RetryStore>,
    config: EventRetryConfig,
    event_handler: Arc<dyn EventHandler>,
    shutdown_rx: watch::Receiver<bool>,
) -> Arc<RetryProcessor> {
    Arc::new(RetryProcessor {
        files_path: PathBuf::from("/tmp/test"),
        event_handler,
        shutdown_rx,
        config,
        store,
    })
}

// ============================================================================
// Backoff - first retry uses initial value
// calculate_backoff(0, 60, 3600) returns 60 (2^0 * initial)
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_backoff_first_retry_uses_initial_value() -> Result<()> {
    setup().await?;

    let post_id = "backoff1st";
    let resource_key = create_resource_key(post_id);
    let store = new_in_memory_store();

    // Create and store a retry event with retry_count = 0
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(
        post_id,
        EventType::Put,
        0, // First retry attempt
        now - 1000,
    );
    store.put(&resource_key, &retry_event).await?;

    // Create processor with initial_backoff_secs = 60
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        create_mock_handler(
            Err(EventProcessorError::Generic("retry error".to_string())),
            post_id,
        ),
        shutdown_rx,
    );

    // Process through the public API
    let _ = processor.run_internal().await;

    // Verify event was re-queued with backoff
    let updated_event = store
        .get(&resource_key)
        .await?
        .expect("Event should be re-queued");

    // First retry (retry_count = 0) should use initial backoff (60 seconds = 60000 ms)
    let expected_next_retry = now + 60_000;
    assert!(
        updated_event.next_retry_at >= expected_next_retry - 1000,
        "First retry should use initial backoff value (2^0 * 60 = 60s)"
    );
    assert!(
        updated_event.next_retry_at <= expected_next_retry + 1000,
        "First retry should use initial backoff value (2^0 * 60 = 60s)"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// Backoff - exponential growth
// calculate_backoff(3, 10, 3600) returns 80 (2^3 * initial)
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_backoff_exponential_growth() -> Result<()> {
    setup().await?;

    let post_id = "backoffexp";
    let resource_key = create_resource_key(post_id);
    let store = new_in_memory_store();

    // Create and store a retry event with retry_count = 3
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(
        post_id,
        EventType::Put,
        3, // Third retry attempt
        now - 1000,
    );
    store.put(&resource_key, &retry_event).await?;

    // Create processor with initial_backoff_secs = 10
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 10, 3600, 60, 3600),
        create_mock_handler(
            Err(EventProcessorError::Generic("retry error".to_string())),
            post_id,
        ),
        shutdown_rx,
    );

    // Process through the public API
    let _ = processor.run_internal().await;

    // Verify event was re-queued with exponential backoff
    let updated_event = store
        .get(&resource_key)
        .await?
        .expect("Event should be re-queued");

    // Retry 3 should have backoff of 2^3 * 10 = 80 seconds = 80000 ms
    let expected_next_retry = now + 80_000;
    assert!(
        updated_event.next_retry_at >= expected_next_retry - 1000,
        "Retry 3 should have backoff of 2^3 * 10 = 80s"
    );
    assert!(
        updated_event.next_retry_at <= expected_next_retry + 1000,
        "Retry 3 should have backoff of 2^3 * 10 = 80s"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// Infrastructure error at max_retries does NOT dead-letter
// This is the key regression test for the P2 Infrastructure bug.
// Even when retry_count >= max_retries, an infrastructure error must NOT be
// dead-lettered — it must be re-queued with retry_count unchanged so the
// event can be retried indefinitely until the infrastructure recovers.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_infrastructure_error_at_max_retries_does_not_dead_letter() -> Result<()> {
    setup().await?;

    let post_id = "inframax";
    let resource_key = create_resource_key(post_id);
    let store = new_in_memory_store();

    // Create a retry event already at max_retries (10)
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(
        post_id,
        EventType::Put,
        10, // At max_retries — would be dead-lettered by application errors
        now - 1000,
    );
    store.put(&resource_key, &retry_event).await?;

    // Create processor with max_retries = 10
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        create_mock_handler(
            Err(EventProcessorError::GraphQueryFailed(
                true, // is_infrastructure = true
                "Database connection failed".to_string(),
            )),
            post_id,
        ),
        shutdown_rx,
    );

    // Process through the public API — infrastructure error must NOT dead-letter
    let result = processor.run_internal().await;
    assert!(
        result.is_err(),
        "Infrastructure error should propagate, not dead-letter"
    );

    // Verify event was NOT removed — it should still be in the queue for retry
    let updated_event = store.get(&resource_key).await?.expect(
        "Event must NOT be dead-lettered; infrastructure errors don't count against max_retries",
    );

    assert_eq!(
        updated_event.retry_count, 10,
        "retry_count must remain 10 (unchanged) — infrastructure errors do not increment retry_count"
    );

    // next_retry_at should have been advanced with backoff
    assert!(
        updated_event.next_retry_at > now,
        "next_retry_at should be in the future after infrastructure error backoff"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// Backoff - capped at max
// Large retry count returns max, never exceeds ceiling
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_backoff_capped_at_max() -> Result<()> {
    setup().await?;

    let post_id = "backoffcap";
    let resource_key = create_resource_key(post_id);
    let store = new_in_memory_store();

    // Create and store a retry event with retry_count = 6
    // 2^6 * 60 = 3840, which exceeds max_backoff_secs (3600), so it should be capped
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(
        post_id,
        EventType::Put,
        6, // Large retry count
        now - 1000,
    );
    store.put(&resource_key, &retry_event).await?;

    // Create processor with initial_backoff_secs = 60, max_backoff_secs = 3600
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        create_mock_handler(
            Err(EventProcessorError::Generic("retry error".to_string())),
            post_id,
        ),
        shutdown_rx,
    );

    // Process through the public API
    let _ = processor.run_internal().await;

    // Verify event was re-queued with capped backoff
    let updated_event = store
        .get(&resource_key)
        .await?
        .expect("Event should be re-queued");

    // Backoff should be capped at max (3600 seconds = 3600000 ms)
    let expected_next_retry = now + 3_600_000;
    assert!(
        updated_event.next_retry_at >= expected_next_retry - 1000,
        "Backoff should be capped at max value (3600s)"
    );
    assert!(
        updated_event.next_retry_at <= expected_next_retry + 1000,
        "Backoff should be capped at max value (3600s)"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// Retry success removes from queue
// Handler returns Ok(()), event is removed from retry index
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_retry_success_removes_from_queue() -> Result<()> {
    setup().await?;

    let post_id = "successrmv";
    let resource_key = create_resource_key(post_id);
    let store = new_in_memory_store();

    // Create and store a retry event
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(
        post_id,
        EventType::Put,
        0,
        now - 1000, // Ready for retry (in the past)
    );
    store.put(&resource_key, &retry_event).await?;

    // Verify event exists in index
    assert!(
        store.get(&resource_key).await?.is_some(),
        "Event should exist in index before processing"
    );

    // Create processor with handler that returns success
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        create_mock_handler(Ok(()), post_id), // Success
        shutdown_rx,
    );

    // Process through the public API
    let _ = processor.run_internal().await;

    // Verify event was removed from index after processing
    assert!(
        store.get(&resource_key).await?.is_none(),
        "Event should be removed from index after successful retry"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// Retry 404 removes from queue
// Handler returns PubkyClientError with 404 message, event is removed (content gone, no point retrying)
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_retry_404_removes_from_queue() -> Result<()> {
    setup().await?;

    let post_id = "r404remove";
    let resource_key = create_resource_key(post_id);
    let event_uri = post_uri_builder(TEST_USER_ID.to_string(), post_id.to_string());
    let store = new_in_memory_store();

    // Create and store a retry event
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(post_id, EventType::Put, 0, now - 1000);
    store.put(&resource_key, &retry_event).await?;

    // Verify event exists in index
    assert!(
        store.get(&resource_key).await?.is_some(),
        "Event should exist in index before processing"
    );

    // Create processor with handler that returns 404
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        create_mock_handler(
            Err(EventProcessorError::PubkyClientError(
                nexus_common::db::PubkyClientError::ClientError(
                    nexus_common::db::PubkyClientErrorKind::NotFound404 { message: event_uri },
                ),
            )),
            post_id,
        ),
        shutdown_rx,
    );

    // Process through the public API
    let _ = processor.run_internal().await;

    // Verify event was removed from index (404 means content is gone)
    assert!(
        store.get(&resource_key).await?.is_none(),
        "Event should be removed from index after 404 error"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// Infrastructure error schedules retry without incrementing retry_count
// Handler returns infrastructure error, event is re-queued WITHOUT incrementing
// retry_count — infrastructure failures must not consume the application-level
// retry budget.  next_retry_at is still advanced via exponential backoff.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_transient_error_schedules_retry() -> Result<()> {
    setup().await?;

    let post_id = "transientr";
    let resource_key = create_resource_key(post_id);
    let store = new_in_memory_store();

    // Create and store a retry event with retry_count = 0
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(
        post_id,
        EventType::Put,
        0, // First retry attempt
        now - 1000,
    );
    store.put(&resource_key, &retry_event).await?;

    // Create processor with handler that returns infrastructure error
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        create_mock_handler(
            Err(EventProcessorError::GraphQueryFailed(
                true, // is_infrastructure = true
                "Database connection failed".to_string(),
            )),
            post_id,
        ),
        shutdown_rx,
    );

    // Process through the public API - this will propagate the infrastructure error
    let result = processor.run_internal().await;
    assert!(result.is_err(), "Infrastructure error should propagate");

    // Verify event was re-queued with retry_count UNCHANGED (infrastructure errors
    // do not consume the application-level retry budget).
    let updated_event = store
        .get(&resource_key)
        .await?
        .expect("Event should be re-queued after transient error");

    assert_eq!(
        updated_event.retry_count, 0,
        "Retry count should remain 0 for infrastructure errors"
    );

    // Verify next_retry_at is set with transient backoff (60 seconds = 60000 ms)
    let expected_next_retry = now + 60_000;
    assert!(
        updated_event.next_retry_at >= expected_next_retry - 1000,
        "Next retry should be scheduled with transient backoff (60s)"
    );
    assert!(
        updated_event.next_retry_at <= expected_next_retry + 1000,
        "Next retry should be scheduled with transient backoff (60s)"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// MissingDependency schedules retry
// Handler returns MissingDependency, event is re-queued with dependency backoff params
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_missing_dependency_schedules_retry() -> Result<()> {
    setup().await?;

    let post_id = "missingdep";
    let resource_key = create_resource_key(post_id);
    let store = new_in_memory_store();

    // Create and store a retry event with retry_count = 0
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(
        post_id,
        EventType::Put,
        0, // First retry attempt
        now - 1000,
    );
    store.put(&resource_key, &retry_event).await?;

    // Create processor with handler that returns MissingDependency
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 300, 18000), // 300s initial for deps
        create_mock_handler(
            Err(EventProcessorError::MissingDependency {
                dependency: vec!["some_dependency".to_string()],
            }),
            post_id,
        ),
        shutdown_rx,
    );

    // Process through the public API
    let _ = processor.run_internal().await;

    // Verify event was re-queued with incremented retry_count
    let updated_event = store
        .get(&resource_key)
        .await?
        .expect("Event should be re-queued after missing dependency error");

    assert_eq!(
        updated_event.retry_count, 1,
        "Retry count should be incremented to 1"
    );

    // Verify next_retry_at is set with dependency backoff (300 seconds = 300000 ms)
    let expected_next_retry = now + 300_000;
    assert!(
        updated_event.next_retry_at >= expected_next_retry - 1000,
        "Next retry should be scheduled with dependency backoff (300s)"
    );
    assert!(
        updated_event.next_retry_at <= expected_next_retry + 1000,
        "Next retry should be scheduled with dependency backoff (300s)"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// Dead-letter after max transient retries
// Event with retry_count >= max_retries for an APPLICATION error is removed
// without retrying.  Infrastructure errors NO LONGER count against max_retries.
// Uses a Generic error (application-level transient) to test the dead-letter path.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_dead_letter_after_max_transient_retries() -> Result<()> {
    setup().await?;

    let post_id = "dltransmax";
    let resource_key = create_resource_key(post_id);
    let store = new_in_memory_store();

    // Create and store a retry event that has exceeded max_retries (10)
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(
        post_id,
        EventType::Put,
        10, // At max_retries
        now - 1000,
    );
    store.put(&resource_key, &retry_event).await?;

    // Verify event exists in index
    assert!(
        store.get(&resource_key).await?.is_some(),
        "Event should exist in index before processing"
    );

    // Create processor with max_retries = 10
    // Uses Generic error (application-level transient, NOT infrastructure)
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        create_mock_handler(
            Err(EventProcessorError::Generic(
                "transient application failure".to_string(),
            )),
            post_id,
        ),
        shutdown_rx,
    );

    // Process through the public API — event should be dead-lettered
    let _ = processor.run_internal().await;

    // Verify event was removed from index (dead-lettered)
    assert!(
        store.get(&resource_key).await?.is_none(),
        "Event should be dead-lettered (removed) after max transient retries"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// Dead-letter after max dependency retries
// retry_count >= max_dependency_retries is removed without retrying
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_dead_letter_after_max_dependency_retries() -> Result<()> {
    setup().await?;

    let post_id = "dldepndmax";
    let resource_key = create_resource_key(post_id);
    let store = new_in_memory_store();

    // Create and store a retry event that has exceeded max_dependency_retries (50)
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(
        post_id,
        EventType::Put,
        50, // At max_dependency_retries
        now - 1000,
    );
    store.put(&resource_key, &retry_event).await?;

    // Verify event exists in index
    assert!(
        store.get(&resource_key).await?.is_some(),
        "Event should exist in index before processing"
    );

    // Create processor with max_dependency_retries = 50
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        create_mock_handler(
            Err(EventProcessorError::MissingDependency {
                dependency: vec!["some_dependency".to_string()],
            }),
            post_id,
        ),
        shutdown_rx,
    );

    // Process through the public API
    let _ = processor.run_internal().await;

    // Verify event was removed from index (dead-lettered)
    assert!(
        store.get(&resource_key).await?.is_none(),
        "Event should be dead-lettered (removed) after max dependency retries"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// Stale sorted set entry cleaned up
// Redis-specific: a sorted-set entry without a matching JSON state should be
// detected and removed by RedisRetryStore::fetch_ready. This test bypasses
// InMemoryRetryStore because the inconsistency doesn't exist in that backend —
// it's Redis layout detail. We exercise RedisRetryStore directly.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_stale_sorted_set_entry_cleaned_up() -> Result<()> {
    setup().await?;

    let post_id = "staleclnup";
    let resource_key = create_resource_key(post_id);

    // Manually add a stale entry to the sorted set only (no JSON state).
    let now = Utc::now().timestamp_millis();
    RetryEvent::put_index_sorted_set(
        &["events"],
        &[(now as f64, &resource_key)],
        Some("RetryManager"),
        None,
    )
    .await?;

    // Sanity: the stale entry is visible in the raw sorted set.
    let raw_before = RetryEvent::fetch_ready(now, None).await?;
    assert!(
        raw_before
            .as_ref()
            .map(|r| r.iter().any(|(key, _)| key == &resource_key))
            .unwrap_or(false),
        "Stale entry should be present in sorted set before cleanup"
    );

    // RedisRetryStore::fetch_ready should silently drop-and-clean stale entries:
    // they're sorted-set members with no corresponding JSON state.
    let store = RedisRetryStore::new();
    let ready = store.fetch_ready(now, None).await?;
    assert!(
        !ready.iter().any(|(key, _)| key == &resource_key),
        "Stale entry {} should be filtered out by RedisRetryStore::fetch_ready",
        resource_key
    );

    // And it should actually be removed from the sorted set (not just filtered).
    let raw_after = RetryEvent::fetch_ready(now, None).await?;
    assert!(
        raw_after
            .as_ref()
            .map(|r| !r.iter().any(|(key, _)| key == &resource_key))
            .unwrap_or(true),
        "Stale entry {} should be removed from sorted set after cleanup",
        resource_key
    );

    Ok(())
}

// ============================================================================
// Shutdown interrupts batch
// Shutdown signal set mid-batch stops processing remaining events and returns Ok(())
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_shutdown_interrupts_batch() -> Result<()> {
    setup().await?;

    // Create multiple retry events
    let num_events = 5;
    let now = Utc::now().timestamp_millis();
    let store = new_in_memory_store();

    for i in 0..num_events {
        let post_id = format!("shutdown{}", i);
        let event_uri = post_uri_builder(TEST_USER_ID.to_string(), post_id);
        let resource_key: RetryEventIndexKey = event_uri.clone();

        let retry_event = RetryEvent {
            retry_count: 0,
            event_type: EventType::Put,
            event_uri,
            next_retry_at: now - 1000,
        };
        store.put(&resource_key, &retry_event).await?;
    }

    // Create processor; shutdown is set before run_internal so nothing is actually
    // processed.
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        create_mock_handler(Ok(()), "shutdown"),
        shutdown_rx,
    );

    // Trigger shutdown before processing
    shutdown_tx.send(true)?;

    // Run the processor - should return Ok(()) immediately due to shutdown
    let result: Result<(), EventProcessorError> = processor.run_internal().await;

    assert!(
        result.is_ok(),
        "Processor should return Ok(()) when shutdown is triggered"
    );

    // Verify events are still in the queue (not processed due to shutdown)
    for i in 0..num_events {
        let resource_key: RetryEventIndexKey =
            post_uri_builder(TEST_USER_ID.to_string(), format!("shutdown{}", i));
        assert!(
            store.get(&resource_key).await?.is_some(),
            "Event {} should still be in queue (not processed due to shutdown)",
            i
        );
    }

    Ok(())
}

// ============================================================================
// Infrastructure error stops batch
// Infrastructure error from processing propagates up, halting the batch
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_infrastructure_error_stops_batch() -> Result<()> {
    setup().await?;

    // Create multiple retry events
    let num_events = 3;
    let now = Utc::now().timestamp_millis();
    let store = new_in_memory_store();

    for i in 0..num_events {
        let post_id = format!("infrastop{}", i);
        let resource_key: RetryEventIndexKey =
            post_uri_builder(TEST_USER_ID.to_string(), post_id.clone());
        let event_uri = post_uri_builder(TEST_USER_ID.to_string(), post_id);

        let retry_event = RetryEvent {
            retry_count: 0,
            event_type: EventType::Put,
            event_uri,
            next_retry_at: now - 1000,
        };
        store.put(&resource_key, &retry_event).await?;
    }

    // Create processor with handler that returns infrastructure error for our events only
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        create_mock_handler(
            Err(EventProcessorError::GraphQueryFailed(
                true, // is_infrastructure = true
                "Critical database failure".to_string(),
            )),
            "infrastop",
        ),
        shutdown_rx,
    );

    // Run the processor - should propagate infrastructure error
    let result: Result<(), EventProcessorError> = processor.run_internal().await;

    // Verify the error propagated up
    assert!(
        result.is_err(),
        "Processor should propagate infrastructure error"
    );

    // Verify the error is an infrastructure error
    let err = result.unwrap_err();
    assert!(
        err.is_infrastructure(),
        "Error should be an infrastructure error"
    );

    // InMemoryRetryStore sorts same-score events lexicographically by key,
    // matching Redis sorted-set semantics. So event 0 is processed first.
    // Infrastructure errors do NOT increment retry_count — they preserve the
    // application-level retry budget.
    let first_key: RetryEventIndexKey =
        post_uri_builder(TEST_USER_ID.to_string(), "infrastop0".to_string());
    let first_event = store
        .get(&first_key)
        .await?
        .expect("First event should still be in queue (re-queued after error)");
    assert_eq!(
        first_event.retry_count, 0,
        "First event should have retry_count unchanged (infrastructure errors do not increment retry_count)"
    );

    // Remaining events should be untouched (retry_count still 0)
    for i in 1..num_events {
        let resource_key: RetryEventIndexKey =
            post_uri_builder(TEST_USER_ID.to_string(), format!("infrastop{}", i));
        let event = store
            .get(&resource_key)
            .await?
            .expect("Event should still be in queue");
        assert_eq!(
            event.retry_count, 0,
            "Event {} should be untouched (retry_count = 0), batch halted before reaching it",
            i
        );
    }

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// Empty batch returns Ok
// No events in queue - processor returns Ok(())
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_empty_batch_returns_ok() -> Result<()> {
    setup().await?;

    // Fresh in-memory store is empty by construction.
    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store,
        create_test_config(10, 50, 60, 3600, 60, 3600),
        create_mock_handler(Ok(()), "empty"),
        shutdown_rx,
    );

    // No events in queue - should return Ok(())
    let result: Result<(), EventProcessorError> = processor.run_internal().await;
    assert!(result.is_ok(), "Empty batch should return Ok(())");

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// DEL event retry success
// DEL events reconstruct correctly and are removed from queue on success
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_del_event_retry_success() -> Result<()> {
    setup().await?;

    let post_id = "delretrys";
    let resource_key = create_resource_key(post_id);
    let store = new_in_memory_store();

    // Create a DEL retry event
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(post_id, EventType::Del, 0, now - 1000);
    store.put(&resource_key, &retry_event).await?;

    // Create processor with handler that returns success
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        create_mock_handler(Ok(()), post_id),
        shutdown_rx,
    );

    let _ = processor.run_internal().await;

    // Verify DEL event was removed from queue after successful processing
    assert!(
        store.get(&resource_key).await?.is_none(),
        "DEL event should be removed from queue after successful retry"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// Non-retryable error removes event immediately
// Handler returns a non-retryable error (e.g. InvalidEventLine), event is
// dead-lettered without incrementing retry_count
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_non_retryable_error_removes_event() -> Result<()> {
    setup().await?;

    let post_id = "nonretrybl";
    let resource_key = create_resource_key(post_id);
    let store = new_in_memory_store();

    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(post_id, EventType::Put, 0, now - 1000);
    store.put(&resource_key, &retry_event).await?;

    // Create processor with handler that returns a non-retryable error
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        create_mock_handler(
            Err(EventProcessorError::InvalidEventLine(
                "malformed data".to_string(),
            )),
            post_id,
        ),
        shutdown_rx,
    );

    let result = processor.run_internal().await;
    assert!(result.is_ok(), "Non-retryable error should not propagate");

    // Event should be removed (dead-lettered immediately, not re-queued)
    assert!(
        store.get(&resource_key).await?.is_none(),
        "Non-retryable error should cause immediate removal from queue"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// Future next_retry_at events are not picked up
// Events with next_retry_at in the future should not be fetched or processed
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_future_events_not_picked_up() -> Result<()> {
    setup().await?;

    let post_id = "futureevnt";
    let resource_key = create_resource_key(post_id);
    let store = new_in_memory_store();

    // Create a retry event scheduled far in the future
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(
        post_id,
        EventType::Put,
        0,
        now + 600_000, // 10 minutes in the future
    );
    store.put(&resource_key, &retry_event).await?;

    // Create processor with handler that would fail if called
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        create_mock_handler(
            Err(EventProcessorError::Generic(
                "should not be called".to_string(),
            )),
            post_id,
        ),
        shutdown_rx,
    );

    let result = processor.run_internal().await;
    assert!(result.is_ok(), "Should return Ok when no ready events");

    // Event should still be in the queue, untouched
    let event = store
        .get(&resource_key)
        .await?
        .expect("Future event should remain in queue");
    assert_eq!(
        event.retry_count, 0,
        "Future event should not have been processed (retry_count unchanged)"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}
