use crate::event_processor::posts::utils::{
    corrupt_post_indexed_at, find_post_details, find_post_uri, restore_post_indexed_at,
};
use crate::event_processor::tags::utils::find_tag_uri;
use crate::event_processor::users::utils::{
    find_cached_user_details, find_user_details, find_user_epoch, find_user_uri, set_user_epoch,
};
use crate::event_processor::utils::default_moderation_tests;
use crate::event_processor::utils::watcher::retrieve_and_handle_event_line;
use crate::service::utils::common::create_mock_handler;
use crate::service::utils::{
    new_in_memory_store, setup, MockHomeserverClient, HS_IDS, TEST_USER_ID,
};
use anyhow::Result;
use chrono::Utc;
use nexus_common::config::EventRetryConfig;
use nexus_common::db::kv::RedisOps;
use nexus_common::db::{exec_single_row, queries};
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::post::PostDetails;
use nexus_common::models::traits::Collection;
use nexus_common::models::user::{Epoch, UserDetails};
use nexus_common::utils::test_utils::{default_ingestor_tests, random_pubky_id};
use nexus_watcher::errors::EventProcessorError;
use nexus_watcher::events::retry::{
    IndexKey, InitialBackoff, RedisRetryStore, RetryEvent, RetryProcessor, RetryScheduler,
    RetryStore, RETRY_MANAGER_EVENTS_INDEX, RETRY_MANAGER_PREFIX,
};
use nexus_watcher::events::EventType;
use nexus_watcher::events::{handlers, DefaultEventHandler, EventHandler};
use nexus_watcher::homeserver::HsErrorKind;
use nexus_watcher::service::{HsEventProcessor, RunOutcome, TEventProcessor};
use pubky_social_specs::legacy_v0::traits::{HashId as _, TimestampId};
use pubky_social_specs::legacy_v0::{
    post_uri_builder, tag_uri_builder as v0_tag_uri_builder, PubkyAppPost, PubkyAppPostKind,
    PubkyAppTag,
};
use pubky_social_specs::traits::{HasPath as _, HashId as _, TimestampId as _};
use pubky_social_specs::{
    post_uri_builder as v1_post_uri_builder, tag_uri_builder as v1_tag_uri_builder,
    PubkySocialPost, PubkySocialPostKind, PubkySocialTag, PubkySocialUser, Root,
};
use std::sync::Arc;
use tokio::sync::watch;

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

/// Origin homeserver carried on test retry events.
const TEST_HOMESERVER_ID: &str = HS_IDS[0];

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
        origin_homeserver_id: TEST_HOMESERVER_ID.to_string(),
        nonce: 0,
    }
}

fn create_index_key(post_id: &str) -> IndexKey {
    IndexKey::for_uri(&post_uri_builder(
        TEST_USER_ID.to_string(),
        post_id.to_string(),
    ))
}

/// Assemble a [`RetryProcessor`] for tests with the given store, config, and handler.
fn build_processor(
    store: Arc<dyn RetryStore>,
    config: EventRetryConfig,
    event_handler: Arc<dyn EventHandler>,
    shutdown_rx: watch::Receiver<bool>,
) -> Arc<RetryProcessor> {
    Arc::new(RetryProcessor {
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
    let resource_key = create_index_key(post_id);
    let store = new_in_memory_store();

    // Create and store a retry event with retry_count = 0
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(
        post_id,
        EventType::Put,
        0, // First retry attempt
        now - 1000,
    );
    store.put(&retry_event).await?;

    // Create processor with initial_backoff_secs = 60
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        create_mock_handler(
            Err(EventProcessorError::Generic("retry error".to_string())),
            Some(post_id),
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
    let resource_key = create_index_key(post_id);
    let store = new_in_memory_store();

    // Create and store a retry event with retry_count = 3
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(
        post_id,
        EventType::Put,
        3, // Third retry attempt
        now - 1000,
    );
    store.put(&retry_event).await?;

    // Create processor with initial_backoff_secs = 10
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 10, 3600, 60, 3600),
        create_mock_handler(
            Err(EventProcessorError::Generic("retry error".to_string())),
            Some(post_id),
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
// Even when retry_count >= max_retries, an error that should not be retried
// right now must NOT be dead-lettered — it must be re-queued with retry_count
// unchanged so the event can be retried indefinitely until the infrastructure recovers.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_infrastructure_error_at_max_retries_does_not_dead_letter() -> Result<()> {
    setup().await?;

    let post_id = "inframax";
    let resource_key = create_index_key(post_id);
    let store = new_in_memory_store();

    // Create a retry event already at max_retries (10)
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(
        post_id,
        EventType::Put,
        10, // At max_retries — would be dead-lettered by application errors
        now - 1000,
    );
    store.put(&retry_event).await?;

    // Create processor with max_retries = 10
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        create_mock_handler(
            Err(EventProcessorError::GraphQueryFailed(
                true, // should_not_retry_now = true
                "Database connection failed".to_string(),
            )),
            Some(post_id),
        ),
        shutdown_rx,
    );

    // Process through the public API — should_not_retry_now error must NOT dead-letter
    let result = processor.run_internal().await;
    assert!(
        result.is_err(),
        "Should-not-retry-now error should propagate, not dead-letter"
    );

    // Verify event was NOT removed — it should still be in the queue for retry
    let updated_event = store.get(&resource_key).await?.expect(
        "Event must NOT be dead-lettered; should-not-retry-now errors don't count against max_retries",
    );

    assert_eq!(
        updated_event.retry_count, 10,
        "retry_count must remain 10 (unchanged) — should-not-retry-now errors do not increment retry_count"
    );

    // next_retry_at should have been advanced with backoff
    assert!(
        updated_event.next_retry_at > now,
        "next_retry_at should be in the future after should-not-retry-now error backoff"
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
    let resource_key = create_index_key(post_id);
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
    store.put(&retry_event).await?;

    // Create processor with initial_backoff_secs = 60, max_backoff_secs = 3600
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        create_mock_handler(
            Err(EventProcessorError::Generic("retry error".to_string())),
            Some(post_id),
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
    let resource_key = create_index_key(post_id);
    let store = new_in_memory_store();

    // Create and store a retry event
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(
        post_id,
        EventType::Put,
        0,
        now - 1000, // Ready for retry (in the past)
    );
    store.put(&retry_event).await?;

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
        create_mock_handler(Ok(()), Some(post_id)),
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
    let resource_key = create_index_key(post_id);
    let event_uri = post_uri_builder(TEST_USER_ID.to_string(), post_id.to_string());
    let store = new_in_memory_store();

    // Create and store a retry event
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(post_id, EventType::Put, 0, now - 1000);
    store.put(&retry_event).await?;

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
            Err(nexus_common::db::PubkyClientError::NotFound404 { message: event_uri }.into()),
            Some(post_id),
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
// Handler returns error that should not be retried right now, event is re-queued
// WITHOUT incrementing retry_count — such failures must not consume the
// application-level retry budget.  next_retry_at is still advanced via
// exponential backoff.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_transient_error_schedules_retry() -> Result<()> {
    setup().await?;

    let post_id = "transientr";
    let resource_key = create_index_key(post_id);
    let store = new_in_memory_store();

    // Create and store a retry event with retry_count = 0
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(
        post_id,
        EventType::Put,
        0, // First retry attempt
        now - 1000,
    );
    store.put(&retry_event).await?;

    // Create processor with handler that returns should_not_retry_now error
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        create_mock_handler(
            Err(EventProcessorError::GraphQueryFailed(
                true, // should_not_retry_now = true
                "Database connection failed".to_string(),
            )),
            Some(post_id),
        ),
        shutdown_rx,
    );

    // Process through the public API - this will propagate the should_not_retry_now error
    let result = processor.run_internal().await;
    assert!(
        result.is_err(),
        "Should-not-retry-now error should propagate"
    );

    // Verify event was re-queued with retry_count UNCHANGED (should-not-retry-now
    // errors do not consume the application-level retry budget).
    let updated_event = store
        .get(&resource_key)
        .await?
        .expect("Event should be re-queued after transient error");

    assert_eq!(
        updated_event.retry_count, 0,
        "Retry count should remain 0 for should-not-retry-now errors"
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
    let resource_key = create_index_key(post_id);
    let store = new_in_memory_store();

    // Create and store a retry event with retry_count = 0
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(
        post_id,
        EventType::Put,
        0, // First retry attempt
        now - 1000,
    );
    store.put(&retry_event).await?;

    // Create processor with handler that returns MissingDependency
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 300, 18000), // 300s initial for deps
        create_mock_handler(
            Err(EventProcessorError::MissingDependency {
                dependency: vec!["some_dependency".to_string()],
            }),
            Some(post_id),
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

    assert_eq!(
        updated_event.origin_homeserver_id, TEST_HOMESERVER_ID,
        "Origin homeserver id must be preserved across reschedule"
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
// without retrying.  Errors we should not retry right now NO LONGER count
// against max_retries.
// Uses a Generic error (application-level transient) to test the dead-letter path.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_dead_letter_after_max_transient_retries() -> Result<()> {
    setup().await?;

    let post_id = "dltransmax";
    let resource_key = create_index_key(post_id);
    let store = new_in_memory_store();

    // Create and store a retry event that has exceeded max_retries (10)
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(
        post_id,
        EventType::Put,
        10, // At max_retries
        now - 1000,
    );
    store.put(&retry_event).await?;

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
            Some(post_id),
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
    let resource_key = create_index_key(post_id);
    let store = new_in_memory_store();

    // Create and store a retry event that has exceeded max_dependency_retries (50)
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(
        post_id,
        EventType::Put,
        50, // At max_dependency_retries
        now - 1000,
    );
    store.put(&retry_event).await?;

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
            Some(post_id),
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
    let resource_key = create_index_key(post_id);

    // Manually add a stale entry to the sorted set only (no JSON state).
    let now = Utc::now().timestamp_millis();
    RetryEvent::put_index_sorted_set(
        &RETRY_MANAGER_EVENTS_INDEX,
        &[(now as f64, resource_key.as_str())],
        Some(RETRY_MANAGER_PREFIX),
        None,
    )
    .await?;

    // Sanity: the stale entry is visible in the raw sorted set.
    let raw_before = RetryEvent::fetch_ready(now, None).await?;
    assert!(
        raw_before.iter().any(|(key, _)| key == &resource_key),
        "Stale entry should be present in sorted set before cleanup"
    );

    // RedisRetryStore::fetch_ready should silently drop-and-clean stale entries:
    // they're sorted-set members with no corresponding JSON state.
    let store = RedisRetryStore::new();
    let ready = store.fetch_ready(now, None).await?;
    assert!(
        !ready.iter().any(|(key, _)| key == &resource_key),
        "Stale entry {resource_key} should be filtered out by RedisRetryStore::fetch_ready"
    );

    // And it should actually be removed from the sorted set (not just filtered).
    let raw_after = RetryEvent::fetch_ready(now, None).await?;
    assert!(
        !raw_after.iter().any(|(key, _)| key == &resource_key),
        "Stale entry {resource_key} should be removed from sorted set after cleanup"
    );

    Ok(())
}

// ============================================================================
// Tombstone cleanup must not remove a member an enqueue just re-created.
// fetch_ready observes a sorted-set member with no JSON state and schedules a
// cleanup; if a fresh enqueue for the same URI lands before that cleanup runs,
// the ZREM must be skipped or the new entry is never fetched again.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_stale_cleanup_keeps_member_recreated_by_enqueue() -> Result<()> {
    setup().await?;

    let post_id = "stalerace1";
    let resource_key = create_index_key(post_id);
    let now = Utc::now().timestamp_millis();

    // Between the fetch that saw a tombstone and its cleanup, a fresh enqueue
    // lands for the same URI.
    let event = create_test_retry_event(post_id, EventType::Put, 0, now - 1000);
    RedisRetryStore::new().put(&event).await?;

    // The deferred cleanup runs with the key it saw as stale.
    RetryEvent::remove_stale_index_entries(std::slice::from_ref(&resource_key)).await?;

    let ready = RedisRetryStore::new().fetch_ready(now, None).await?;
    assert!(
        ready.iter().any(|(key, _)| key == &resource_key),
        "entry re-created by an enqueue must survive the stale cleanup"
    );

    Ok(())
}

// ============================================================================
// Nonce guard survives an overwrite (Redis-backed)
// Regression test: retry entries are keyed by hash(URI) only, so a
// newer event enqueued for the same URI overwrites the entry while the retry
// processor may still hold the old one in memory. The stale event's
// conditional remove/reschedule (Lua compare-and-act) must be a no-op and the
// newer entry must remain fetchable. Exercises RedisRetryStore directly since
// the atomicity lives in the Redis Lua script.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_redis_stale_conditional_ops_do_not_clobber_newer_entry() -> Result<()> {
    setup().await?;

    let post_id = "noncegrd01";
    let resource_key = create_index_key(post_id);
    let store = RedisRetryStore::new();

    let now = Utc::now().timestamp_millis();

    // E1: a PUT enqueued and then snapshotted by the retry processor.
    let mut e1 = create_test_retry_event(post_id, EventType::Put, 0, now - 1000);
    e1.nonce = 100;
    store.put(&e1).await?;

    // E2: a live processor transiently fails a DEL for the same URI and
    // enqueues it, overwriting the hash(URI) slot with a fresh nonce.
    let mut e2 = create_test_retry_event(post_id, EventType::Del, 0, now - 500);
    e2.nonce = 200;
    store.put(&e2).await?;

    // E1's retry succeeds; its conditional removal must not act.
    assert!(
        !store.remove_if(&resource_key, e1.nonce).await?,
        "remove_if with a stale nonce must be a no-op"
    );

    // A stale reschedule of E1 must not clobber E2 either.
    let mut rescheduled_e1 = e1.clone();
    rescheduled_e1.retry_count = 1;
    rescheduled_e1.next_retry_at = now + 60_000;
    assert!(
        !store.put_if(&rescheduled_e1, e1.nonce).await?,
        "put_if with a stale nonce must be a no-op"
    );

    // E2 survives untouched and is still fetchable.
    let stored = store
        .get(&resource_key)
        .await?
        .expect("newer entry must survive stale conditional ops");
    assert_eq!(stored.nonce, 200, "stored entry must still be E2");
    assert_eq!(stored.event_type, EventType::Del);
    assert_eq!(stored.retry_count, 0);
    let ready = store.fetch_ready(now, None).await?;
    assert!(
        ready
            .iter()
            .any(|(key, event)| key == &resource_key && event.nonce == 200),
        "newer entry must still be returned by fetch_ready"
    );

    // Matching nonce acts: reschedule E2, then remove it (also cleans up).
    let mut rescheduled_e2 = stored.clone();
    rescheduled_e2.retry_count = 1;
    assert!(
        store.put_if(&rescheduled_e2, 200).await?,
        "put_if with the matching nonce must act"
    );
    assert!(
        store.remove_if(&resource_key, 200).await?,
        "remove_if with the matching nonce must act"
    );
    assert!(
        store.get(&resource_key).await?.is_none(),
        "entry must be gone after matching-nonce removal"
    );
    // The Lua script's ZREM must also drop the sorted-set member; a key/member
    // mismatch inside the script would leave a stale index entry behind while
    // the JSON state is gone.
    assert!(
        !RetryEvent::check_index_key(&resource_key).await?,
        "sorted-set member must be gone after matching-nonce removal"
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

        let retry_event = RetryEvent {
            retry_count: 0,
            event_type: EventType::Put,
            event_uri,
            next_retry_at: now - 1000,
            origin_homeserver_id: TEST_HOMESERVER_ID.to_string(),
            nonce: 0,
        };
        store.put(&retry_event).await?;
    }

    // Create processor; shutdown is set before run_internal so nothing is actually
    // processed.
    let handler = create_mock_handler(Ok(()), Some("shutdown"));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        handler.clone(),
        shutdown_rx,
    );

    // Trigger shutdown before processing
    shutdown_tx.send(true)?;

    // Run the processor - should return Ok(()) immediately due to shutdown
    let result: Result<RunOutcome, EventProcessorError> = processor.run_internal().await;

    assert!(
        result.is_ok(),
        "Processor should return Ok(()) when shutdown is triggered"
    );

    // Handler must not be called — shutdown short-circuits before any processing.
    assert_eq!(
        handler.get_handle_count(),
        0,
        "Handler must not be called when shutdown is triggered before processing"
    );

    // Verify events are still in the queue (not processed due to shutdown)
    for i in 0..num_events {
        let resource_key = IndexKey::for_uri(&post_uri_builder(
            TEST_USER_ID.to_string(),
            format!("shutdown{}", i),
        ));
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
// Error that should not be retried right now from processing propagates up,
// halting the batch
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
        let event_uri = post_uri_builder(TEST_USER_ID.to_string(), post_id);

        let retry_event = RetryEvent {
            retry_count: 0,
            event_type: EventType::Put,
            event_uri,
            next_retry_at: now - 1000,
            origin_homeserver_id: TEST_HOMESERVER_ID.to_string(),
            nonce: 0,
        };
        store.put(&retry_event).await?;
    }

    // Create processor with handler that returns should_not_retry_now error for our events only
    let handler = create_mock_handler(
        Err(EventProcessorError::GraphQueryFailed(
            true, // should_not_retry_now = true
            "Critical database failure".to_string(),
        )),
        Some("infrastop"),
    );
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        handler.clone(),
        shutdown_rx,
    );

    // Run the processor - should propagate should_not_retry_now error
    let result: Result<RunOutcome, EventProcessorError> = processor.run_internal().await;

    // Verify the error propagated up
    assert!(
        result.is_err(),
        "Processor should propagate should-not-retry-now error"
    );

    // Handler called exactly once — should_not_retry_now error halted the batch
    // after the first event, so remaining events were never reached.
    assert_eq!(
        handler.get_handle_count(),
        1,
        "Handler must be called exactly once — batch stopped on should-not-retry-now error"
    );

    // Verify the error is a should-not-retry-now error
    let err = result.unwrap_err();
    assert!(
        err.should_not_retry_now(),
        "Error should be a should-not-retry-now error"
    );

    // Should-not-retry-now errors do NOT increment retry_count — they preserve the
    // application-level retry budget.
    let first_key = IndexKey::for_uri(&post_uri_builder(
        TEST_USER_ID.to_string(),
        "infrastop0".to_string(),
    ));
    let first_event = store
        .get(&first_key)
        .await?
        .expect("First event should still be in queue (re-queued after error)");
    assert_eq!(
        first_event.retry_count, 0,
        "First event should have retry_count unchanged (should-not-retry-now errors do not increment retry_count)"
    );

    // Remaining events should be untouched (retry_count still 0)
    for i in 1..num_events {
        let resource_key = IndexKey::for_uri(&post_uri_builder(
            TEST_USER_ID.to_string(),
            format!("infrastop{}", i),
        ));
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
    let handler = create_mock_handler(Ok(()), Some("empty"));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store,
        create_test_config(10, 50, 60, 3600, 60, 3600),
        handler.clone(),
        shutdown_rx,
    );

    // No events in queue - should return Ok(())
    let result: Result<RunOutcome, EventProcessorError> = processor.run_internal().await;
    assert!(result.is_ok(), "Empty batch should return Ok(())");

    // No events, handler must never be called.
    assert_eq!(
        handler.get_handle_count(),
        0,
        "Handler must not be called when no events are in queue"
    );

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
    let resource_key = create_index_key(post_id);
    let store = new_in_memory_store();

    // Create a DEL retry event
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(post_id, EventType::Del, 0, now - 1000);
    store.put(&retry_event).await?;

    // Create processor with handler that returns success
    let handler = create_mock_handler(Ok(()), Some(post_id));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        handler.clone(),
        shutdown_rx,
    );

    let _ = processor.run_internal().await;

    // Handler called once for the DEL event.
    assert_eq!(
        handler.get_handle_count(),
        1,
        "Handler must be called exactly once for the DEL event"
    );

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
    let resource_key = create_index_key(post_id);
    let store = new_in_memory_store();

    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(post_id, EventType::Put, 0, now - 1000);
    store.put(&retry_event).await?;

    // Create processor with handler that returns a non-retryable error
    let handler = create_mock_handler(
        Err(EventProcessorError::InvalidEventLine(
            "malformed data".to_string(),
        )),
        Some(post_id),
    );
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        handler.clone(),
        shutdown_rx,
    );

    let result = processor.run_internal().await;
    assert!(result.is_ok(), "Non-retryable error should not propagate");

    // Handler called once for the event before dead-lettering.
    assert_eq!(
        handler.get_handle_count(),
        1,
        "Handler must be called exactly once before non-retryable error removes event"
    );

    // Event should be removed (dead-lettered immediately, not re-queued)
    assert!(
        store.get(&resource_key).await?.is_none(),
        "Non-retryable error should cause immediate removal from queue"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// Batch continues after a single event fails
// A retryable application error on one event must not halt the batch — later
// events still need to be processed.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_batch_continues_after_single_failure() -> Result<()> {
    setup().await?;

    // Both events share the same next_retry_at so they are fetched in the same
    // batch. The test is order-independent: regardless of which event is
    // processed first, the failing one is re-queued and the succeeding one is
    // removed — proving the batch continued past the failure.
    let failing_post_id = "failbatch1";
    let succeeding_post_id = "okbatch2";
    let failing_key = create_index_key(failing_post_id);
    let succeeding_key = create_index_key(succeeding_post_id);

    let store = new_in_memory_store();
    let now = Utc::now().timestamp_millis();
    store
        .put(&create_test_retry_event(
            failing_post_id,
            EventType::Put,
            0,
            now - 1000,
        ))
        .await?;
    store
        .put(&create_test_retry_event(
            succeeding_post_id,
            EventType::Put,
            0,
            now - 1000,
        ))
        .await?;

    // MockEventHandler's `target_uri_substring` scopes the error to the failing
    // post_id; the succeeding event's URI doesn't match and so falls through to
    // Ok(()).
    let handler = create_mock_handler(
        Err(EventProcessorError::Generic(
            "first handler fails".to_string(),
        )),
        Some(failing_post_id),
    );
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        handler.clone(),
        shutdown_rx,
    );

    let result = processor.run_internal().await;
    assert!(
        result.is_ok(),
        "Retryable application error must not stop the batch"
    );

    // Handler called twice — once for each event — proving the batch
    // continued past the first failure.
    assert_eq!(
        handler.get_handle_count(),
        2,
        "Handler must be called for both events — batch continued past failure"
    );

    // First event failed with a retryable Generic error — re-queued with
    // retry_count incremented.
    let requeued = store
        .get(&failing_key)
        .await?
        .expect("Failing event should remain in queue for retry");
    assert_eq!(
        requeued.retry_count, 1,
        "Failing event should have retry_count incremented after retryable failure"
    );

    // Second event was reached despite the first failing, and its handler
    // returned Ok(()), so the entry must have been removed.
    assert!(
        store.get(&succeeding_key).await?.is_none(),
        "Processor must continue past a failed event and process the next one"
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
    let resource_key = create_index_key(post_id);
    let store = new_in_memory_store();

    // Create a retry event scheduled far in the future
    let now = Utc::now().timestamp_millis();
    let retry_event = create_test_retry_event(
        post_id,
        EventType::Put,
        0,
        now + 600_000, // 10 minutes in the future
    );
    store.put(&retry_event).await?;

    // Create processor with handler that would fail if called
    let handler = create_mock_handler(
        Err(EventProcessorError::Generic(
            "should not be called".to_string(),
        )),
        Some(post_id),
    );
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        handler.clone(),
        shutdown_rx,
    );

    let result = processor.run_internal().await;
    assert!(result.is_ok(), "Should return Ok when no ready events");

    // Handler must never be called — no ready events in the batch.
    assert_eq!(
        handler.get_handle_count(),
        0,
        "Handler must not be called when no events are ready"
    );

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

// ============================================================================
// Skipped-by-design entry is removed without a handler call
// A `social/v1` file line left in the queue re-parses to `Skipped`: the
// processor drops it unfetched and still handles the v0 line next to it.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_skipped_entry_is_removed_without_handler_call() -> Result<()> {
    setup().await?;

    let store = new_in_memory_store();
    let now = Utc::now().timestamp_millis();

    // A v0 post line and, next to it, a `social/v1` file line, which no handler
    // takes until v1 files are supported; both ready now.
    let post_id = "skipnextv0";
    let v0_event = create_test_retry_event(post_id, EventType::Put, 0, now - 1000);
    let v0_uri = v0_event.event_uri.clone();
    let v1_uri =
        format!("pubky://{TEST_USER_ID}/pub/social/v1/files/8Z8CWH8NVYQY39ZEBFGKQWWEKG.jpg");
    let v1_event = RetryEvent {
        event_uri: v1_uri.clone(),
        ..v0_event.clone()
    };
    store.put(&v0_event).await?;
    store.put(&v1_event).await?;

    let handler = create_mock_handler(Ok(()), None);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        handler.clone(),
        shutdown_rx,
    );

    processor
        .run_internal()
        .await
        .expect("a skipped entry must not fail the batch");

    assert_eq!(
        handler.get_handled_uris(),
        vec![v0_uri],
        "Handler must see the v0 line only"
    );
    assert!(
        store.get(&IndexKey::for_uri(&v1_uri)).await?.is_none(),
        "Skipped entry must be removed from the queue"
    );
    assert!(
        store.get(&create_index_key(post_id)).await?.is_none(),
        "Handled v0 entry must be removed from the queue"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// Entry the epoch gate ignores is removed
// A `pubky.app` line queued for a user since moved to `v1` goes through the real
// handler: the gate ignores it, so the entry is removed and nothing is indexed.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_gated_entry_is_removed_and_nothing_indexed() -> Result<()> {
    setup().await?;

    let user = random_pubky_id();
    let user_id = user.to_string();
    UserDetails::from_pubky(user).put_to_graph().await?;
    set_user_epoch(&user_id, "v1").await?;

    // The homeserver still serves the post: only the gate keeps it out
    let post = PubkyAppPost {
        content: "Retry:EpochGate:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
        lock: None,
    };
    let post_id = post.create_id();
    let event_uri = post_uri_builder(user_id.clone(), post_id.clone());
    let client = Arc::new(
        MockHomeserverClient::default().with_resource(&event_uri, serde_json::to_vec(&post)?),
    );
    let files = tempfile::TempDir::new()?;
    let store = new_in_memory_store();
    let handler: Arc<dyn EventHandler> = Arc::new(DefaultEventHandler::new(
        default_moderation_tests(),
        default_ingestor_tests(),
        client,
        files.path().to_path_buf(),
        store.clone(),
    ));

    let retry_event = RetryEvent {
        retry_count: 0,
        event_type: EventType::Put,
        event_uri: event_uri.clone(),
        next_retry_at: Utc::now().timestamp_millis() - 1000,
        origin_homeserver_id: TEST_HOMESERVER_ID.to_string(),
        nonce: 0,
    };
    store.put(&retry_event).await?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        handler,
        shutdown_rx,
    );

    processor
        .run_internal()
        .await
        .expect("an ignored entry must not fail the batch");

    assert!(
        store.get(&IndexKey::for_uri(&event_uri)).await?.is_none(),
        "Ignored entry must be removed from the queue"
    );
    assert!(
        PostDetails::get_from_graph(&user_id, &post_id)
            .await?
            .is_none(),
        "Ignored entry must index nothing"
    );

    exec_single_row(queries::del::delete_user(&user_id)).await?;
    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// `social/v1` entry of a `v0` user is removed
// A v1 line queued for a user still at `v0` goes through the real handler:
// the gate ignores it, so the entry is removed and nothing is indexed.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_social_v1_entry_of_a_v0_user_is_removed_and_nothing_indexed() -> Result<()> {
    setup().await?;

    let user = random_pubky_id();
    let user_id = user.to_string();
    // No epoch on the node: the user reads as `v0`
    UserDetails::from_pubky(user).put_to_graph().await?;

    // The homeserver still serves the post version: only the gate keeps it out
    let post = PubkySocialPost::new(
        "Retry:EpochGate:SocialV1".to_string(),
        PubkySocialPostKind::Note,
        None,
        None,
        vec![],
    );
    let post_id = post.create_id();
    let path = PubkySocialPost::create_path_in(Root::Pub, &post_id, &post_id, None);
    let event_uri = format!("pubky://{user_id}{path}");
    let client = Arc::new(
        MockHomeserverClient::default().with_resource(&event_uri, serde_json::to_vec(&post)?),
    );
    let files = tempfile::TempDir::new()?;
    let store = new_in_memory_store();
    let handler: Arc<dyn EventHandler> = Arc::new(DefaultEventHandler::new(
        default_moderation_tests(),
        default_ingestor_tests(),
        client,
        files.path().to_path_buf(),
        store.clone(),
    ));

    let retry_event = RetryEvent {
        retry_count: 0,
        event_type: EventType::Put,
        event_uri: event_uri.clone(),
        next_retry_at: Utc::now().timestamp_millis() - 1000,
        origin_homeserver_id: TEST_HOMESERVER_ID.to_string(),
        nonce: 0,
    };
    store.put(&retry_event).await?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        handler,
        shutdown_rx,
    );

    processor
        .run_internal()
        .await
        .expect("an ignored entry must not fail the batch");

    assert!(
        store.get(&IndexKey::for_uri(&event_uri)).await?.is_none(),
        "Ignored entry must be removed from the queue"
    );
    assert!(
        PostDetails::get_from_graph(&user_id, &post_id)
            .await?
            .is_none(),
        "Ignored entry must index nothing"
    );

    exec_single_row(queries::del::delete_user(&user_id)).await?;
    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// A queued `social/v1` profile flips its `v0` user
// A v1 profile line queued for a user still at `v0` goes through the real
// handler: the user moves to `v1` in the graph, then the profile is indexed and
// cached at `v1`, and the entry is removed.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_social_v1_profile_entry_flips_a_v0_user() -> Result<()> {
    setup().await?;

    let user = random_pubky_id();
    let user_id = user.to_string();
    // No epoch on the node: the user reads as `v0`
    UserDetails::from_pubky(user.clone()).put_to_graph().await?;

    let profile = PubkySocialUser::new("Retry:EpochGate:Flip".to_string(), None, None, None, None);
    let event_uri = format!("pubky://{user_id}{}", PubkySocialUser::create_path());
    let client = Arc::new(
        MockHomeserverClient::default().with_resource(&event_uri, serde_json::to_vec(&profile)?),
    );
    let files = tempfile::TempDir::new()?;
    let store = new_in_memory_store();
    let handler: Arc<dyn EventHandler> = Arc::new(DefaultEventHandler::new(
        default_moderation_tests(),
        default_ingestor_tests(),
        client,
        files.path().to_path_buf(),
        store.clone(),
    ));

    let retry_event = RetryEvent {
        retry_count: 0,
        event_type: EventType::Put,
        event_uri: event_uri.clone(),
        next_retry_at: Utc::now().timestamp_millis() - 1000,
        origin_homeserver_id: TEST_HOMESERVER_ID.to_string(),
        nonce: 0,
    };
    store.put(&retry_event).await?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(10, 50, 60, 3600, 60, 3600),
        handler,
        shutdown_rx,
    );

    processor
        .run_internal()
        .await
        .expect("the flip must not fail the batch");

    assert_eq!(find_user_epoch(&user_id).await.as_deref(), Some("v1"));
    let cached = find_cached_user_details(&user_id).await?;
    assert_eq!(cached.epoch, Epoch::V1);
    assert_eq!(cached.name, "Retry:EpochGate:Flip");
    assert!(
        store.get(&IndexKey::for_uri(&event_uri)).await?.is_none(),
        "Handled entry must be removed from the queue"
    );

    handlers::user::del(user).await?;
    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// The flip step runs before the profile fetch
// A `social/v1` profile PUT of a user at `v0` moves them to `v1` first: the
// internal migration of their indexed data, then the epoch write. Only then is
// the profile fetched, as the `v1` edit it now is, so the flip does not wait for
// the homeserver and a failed migration step is queued without limit while the
// user is `v0`. These tests go through the real handler against the local graph,
// with a homeserver client stub that answers the profile fetch.
// ============================================================================

/// The real event handler, fetching through `client` (owned, or an `Arc` a runner shares)
/// and reading the retry queue `store`; `files` must outlive it.
fn default_handler(
    client: impl Into<Arc<MockHomeserverClient>>,
    files: &tempfile::TempDir,
    store: Arc<dyn RetryStore>,
) -> Arc<dyn EventHandler> {
    let client: Arc<MockHomeserverClient> = client.into();
    Arc::new(DefaultEventHandler::new(
        default_moderation_tests(),
        default_ingestor_tests(),
        client,
        files.path().to_path_buf(),
        store,
    ))
}

/// A PUT entry of `event_uri` at `retry_count` attempts, ready for retry.
fn ready_put_entry(event_uri: &str, retry_count: u32) -> RetryEvent {
    RetryEvent {
        retry_count,
        event_type: EventType::Put,
        event_uri: event_uri.to_string(),
        next_retry_at: Utc::now().timestamp_millis() - 1000,
        origin_homeserver_id: TEST_HOMESERVER_ID.to_string(),
        nonce: 0,
    }
}

/// The URI of `user_id`'s `social/v1` profile.
fn social_v1_profile_uri(user_id: &str) -> String {
    format!("pubky://{user_id}{}", PubkySocialUser::create_path())
}

/// A homeserver outage: the fetch answers 503.
fn outage() -> HsErrorKind {
    HsErrorKind::ServerError(503, "service unavailable".to_string())
}

const MAX_RETRIES: u32 = 10;

/// A `pubky.app` tag by `tagger_id` on `target` and its URI.
fn pubky_app_tag(tagger_id: &str, target: &str, label: &str) -> (PubkyAppTag, String) {
    let tag = PubkyAppTag {
        uri: target.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let uri = v0_tag_uri_builder(tagger_id.to_string(), tag.create_id());
    (tag, uri)
}

/// The flip's first attempt with the homeserver down: the flip step lands, so the user is
/// `v1` with their post and their tag at their `v1` addresses, and the profile PUT is queued
/// with the transient class as the `v1` edit it now is. The homeserver back, the entry's retry
/// fetches and indexes the `social/v1` profile, the details cache shows it at `v1`, and the
/// entry leaves the queue. (The `info` line is asserted in the handler's own tests, where
/// the crate's logs can be read.)
#[tokio_shared_rt::test(shared)]
async fn test_a_5xx_at_the_fetch_flips_the_user_and_queues_the_profile_put_as_a_v1_edit(
) -> Result<()> {
    setup().await?;

    let user = random_pubky_id();
    let user_id = user.to_string();
    // No epoch on the node: the user reads as `v0`
    UserDetails::from_pubky(user).put_to_graph().await?;
    let (post, post_id, post_uri) = pubky_app_post(&user_id, "Retry:Flip:Outage:Post");
    let label = "flipoutage";
    let (tag, tag_uri) = pubky_app_tag(&user_id, &post_uri, label);
    let flip_uri = social_v1_profile_uri(&user_id);

    // The user's post and tag, indexed at `pubky.app`; then the flip, whose fetch answers 503
    let store = new_in_memory_store();
    let files = tempfile::TempDir::new()?;
    let client = Arc::new(
        MockHomeserverClient::default()
            .with_lines(batch([format!("PUT {flip_uri}")]))
            .with_resource(&post_uri, serde_json::to_vec(&post)?)
            .with_resource(&tag_uri, serde_json::to_vec(&tag)?)
            .with_resource_error(&flip_uri, outage()),
    );
    let handler = default_handler(client.clone(), &files, store.clone());
    retrieve_and_handle_event_line(&format!("PUT {post_uri}"), handler.clone()).await?;
    retrieve_and_handle_event_line(&format!("PUT {tag_uri}"), handler.clone()).await?;
    let runner = primary_runner(client, handler, store.clone()).await?;

    let now = Utc::now().timestamp_millis();
    runner
        .run_internal()
        .await
        .expect("a 5xx at the fetch must not fail the batch");

    assert_eq!(find_user_epoch(&user_id).await.as_deref(), Some("v1"));
    let v1_post_uri = v1_post_uri_builder(user_id.clone(), post_id.clone());
    assert_eq!(
        find_post_uri(&user_id, &post_id).await,
        Some(v1_post_uri.clone()),
        "the post is at its v1 address"
    );
    let v1_tag_id = PubkySocialTag::new(v1_post_uri, label.to_string()).create_id();
    assert_eq!(
        find_tag_uri(&user_id, &v1_tag_id).await,
        Some(v1_tag_uri_builder(user_id.clone(), v1_tag_id)),
        "the tag is at its v1 address"
    );
    let queued = store
        .get(&IndexKey::for_uri(&flip_uri))
        .await?
        .expect("the profile PUT must be queued");
    assert_eq!(queued.event_type, EventType::Put);
    assert_eq!(queued.retry_count, 0);
    assert!(
        (now + TRANSIENT_MS..now + MISSING_DEP_MS).contains(&queued.next_retry_at),
        "the profile PUT is queued with the transient class"
    );
    assert_eq!(
        find_user_details(&user_id).await?.name,
        user_id,
        "the previous profile stays until the fetch"
    );

    // The homeserver is back: the same entry, due again, indexes the profile and leaves the queue
    store
        .put(&RetryEvent {
            next_retry_at: now - 1000,
            ..queued
        })
        .await?;
    let profile = PubkySocialUser::new("Retry:Flip:Outage".to_string(), None, None, None, None);
    let handler = default_handler(
        MockHomeserverClient::default().with_resource(&flip_uri, serde_json::to_vec(&profile)?),
        &files,
        store.clone(),
    );
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(MAX_RETRIES, 50, 60, 3600, 60, 3600),
        handler,
        shutdown_rx,
    );

    processor
        .run_internal()
        .await
        .expect("the profile PUT must not fail the batch");

    assert!(
        store.get(&IndexKey::for_uri(&flip_uri)).await?.is_none(),
        "the indexed entry must leave the queue"
    );
    assert_eq!(find_user_uri(&user_id).await, Some(flip_uri.clone()));
    let cached = find_cached_user_details(&user_id).await?;
    assert_eq!(cached.epoch, Epoch::V1);
    assert_eq!(cached.name, "Retry:Flip:Outage");

    exec_single_row(queries::del::delete_post(&user_id, &post_id)).await?;
    exec_single_row(queries::del::delete_user(&user_id)).await?;
    Ok(())
}

/// A `v0` user's entry at `max_retries` whose flip step lands and whose fetch answers 5xx:
/// the user flips, and the entry is dead-lettered as the `v1` edit it then is. The fetch after
/// the epoch write is a bounded `v1` edit: the previous profile stays until the user's next
/// profile write.
#[tokio_shared_rt::test(shared)]
async fn test_a_v0_users_entry_at_max_retries_with_a_5xx_at_the_fetch_flips_and_is_dead_lettered(
) -> Result<()> {
    setup().await?;

    let user = random_pubky_id();
    let user_id = user.to_string();
    // No epoch on the node: the user reads as `v0`
    UserDetails::from_pubky(user).put_to_graph().await?;

    let flip_uri = social_v1_profile_uri(&user_id);
    let store = new_in_memory_store();
    store.put(&ready_put_entry(&flip_uri, MAX_RETRIES)).await?;

    let files = tempfile::TempDir::new()?;
    let handler = default_handler(
        MockHomeserverClient::default().with_resource_error(&flip_uri, outage()),
        &files,
        store.clone(),
    );
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(MAX_RETRIES, 50, 60, 3600, 60, 3600),
        handler,
        shutdown_rx,
    );

    processor
        .run_internal()
        .await
        .expect("a dead-lettered entry must not fail the batch");

    assert_eq!(
        find_user_epoch(&user_id).await.as_deref(),
        Some("v1"),
        "the flip step landed before the fetch"
    );
    assert!(
        store.get(&IndexKey::for_uri(&flip_uri)).await?.is_none(),
        "a v1 user's profile edit is dead-lettered at max_retries"
    );
    assert_eq!(find_user_details(&user_id).await?.name, user_id);

    exec_single_row(queries::del::delete_user(&user_id)).await?;
    Ok(())
}

/// A profile the fetch cannot index, a 404 or an invalid body, after the flip step landed: the
/// event is dropped as a `v1` user's and never retried, the user is `v1` with the previous
/// profile, and their `pubky.app` events are ignored from then on.
#[tokio_shared_rt::test(shared)]
async fn test_a_profile_the_fetch_cannot_index_leaves_a_v1_user_with_the_previous_profile(
) -> Result<()> {
    setup().await?;

    for (case, body) in [
        ("a 404", None),
        ("an invalid profile", Some(b"not json".to_vec())),
    ] {
        let user = random_pubky_id();
        let user_id = user.to_string();
        // No epoch on the node: the user reads as `v0`
        UserDetails {
            name: "Retry:Flip:Previous".to_string(),
            ..UserDetails::from_pubky(user)
        }
        .put_to_graph()
        .await?;
        let profile_uri = social_v1_profile_uri(&user_id);
        let (post, post_id, post_uri) = pubky_app_post(&user_id, "Retry:Flip:Previous:Post");

        // The mock client answers 404 for any URI it does not serve
        let mut client =
            MockHomeserverClient::default().with_resource(&post_uri, serde_json::to_vec(&post)?);
        if let Some(body) = body {
            client = client.with_resource(&profile_uri, body);
        }
        let files = tempfile::TempDir::new()?;
        let store = new_in_memory_store();
        let handler = default_handler(client, &files, store.clone());
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        let processor = build_processor(
            store.clone(),
            create_test_config(MAX_RETRIES, 50, 60, 3600, 60, 3600),
            handler,
            shutdown_rx,
        );

        store.put(&ready_put_entry(&profile_uri, 0)).await?;
        processor
            .clone()
            .run_internal()
            .await
            .unwrap_or_else(|e| panic!("{case}: a dropped profile must not fail the batch: {e}"));

        assert_eq!(
            find_user_epoch(&user_id).await.as_deref(),
            Some("v1"),
            "{case}: the flip step landed before the fetch"
        );
        assert_eq!(
            find_user_details(&user_id).await?.name,
            "Retry:Flip:Previous",
            "{case}: the previous profile stays"
        );
        assert!(
            store.get(&IndexKey::for_uri(&profile_uri)).await?.is_none(),
            "{case}: a profile that cannot be indexed is never retried"
        );

        // At `v1`: a `pubky.app` post from the same user is ignored
        store.put(&ready_put_entry(&post_uri, 0)).await?;
        processor
            .run_internal()
            .await
            .unwrap_or_else(|e| panic!("{case}: an ignored post must not fail the batch: {e}"));
        assert!(
            PostDetails::get_from_graph(&user_id, &post_id)
                .await?
                .is_none(),
            "{case}: a v1 user's pubky.app post is ignored"
        );
        assert!(store.get(&IndexKey::for_uri(&post_uri)).await?.is_none());

        exec_single_row(queries::del::delete_user(&user_id)).await?;
    }
    Ok(())
}

/// A failed migration step through the real handler: the handler's error is `FlipFailed`
/// naming the step and the cause, and the user stays at `v0` with their profile unfetched and
/// their post's content intact. The posts step's graph write landed before its read failed;
/// the retry runs the step whole over it, flips the user and indexes the profile. (The log
/// lines are asserted in the handler's own tests, where the crate's logs can be read.)
#[tokio_shared_rt::test(shared)]
async fn test_a_failed_migration_step_comes_out_of_the_handler_as_flip_failed() -> Result<()> {
    setup().await?;

    let user = random_pubky_id();
    let user_id = user.to_string();
    // No epoch on the node: the user reads as `v0`
    UserDetails::from_pubky(user).put_to_graph().await?;
    let (post, post_id, post_uri) = pubky_app_post(&user_id, "Retry:FlipFailed:Post");
    let flip_uri = social_v1_profile_uri(&user_id);
    let profile = PubkySocialUser::new("Retry:FlipFailed".to_string(), None, None, None, None);

    let store = new_in_memory_store();
    let files = tempfile::TempDir::new()?;
    let handler = default_handler(
        MockHomeserverClient::default()
            .with_resource(&post_uri, serde_json::to_vec(&post)?)
            .with_resource(&flip_uri, serde_json::to_vec(&profile)?),
        &files,
        store.clone(),
    );
    retrieve_and_handle_event_line(&format!("PUT {post_uri}"), handler.clone()).await?;
    let indexed_at = find_post_details(&user_id, &post_id).await?.indexed_at;
    corrupt_post_indexed_at(&user_id, &post_id).await?;

    let err = retrieve_and_handle_event_line(&format!("PUT {flip_uri}"), handler.clone())
        .await
        .expect_err("a failed step must fail the flip");

    match &err {
        EventProcessorError::FlipFailed { step, source } => {
            assert_eq!(*step, "posts");
            assert!(
                matches!(**source, EventProcessorError::GraphQueryFailed(false, _)),
                "the cause is kept: {source:?}"
            );
        }
        other => panic!("expected FlipFailed, got {other:?}"),
    }
    assert_eq!(
        find_user_epoch(&user_id).await,
        None,
        "the user stays at v0"
    );
    assert_eq!(
        find_user_details(&user_id).await?.name,
        user_id,
        "the profile is not fetched before the step lands"
    );
    assert!(
        store.get(&IndexKey::for_uri(&flip_uri)).await?.is_none(),
        "the handler queues nothing itself; the runner does"
    );

    // The row reads again: the retry runs the step whole and the flip lands
    restore_post_indexed_at(&user_id, &post_id, indexed_at).await?;
    assert_eq!(
        find_post_details(&user_id, &post_id).await?.content,
        "Retry:FlipFailed:Post"
    );
    retrieve_and_handle_event_line(&format!("PUT {flip_uri}"), handler).await?;

    assert_eq!(find_user_epoch(&user_id).await.as_deref(), Some("v1"));
    assert_eq!(find_user_uri(&user_id).await, Some(flip_uri));
    assert_eq!(find_user_details(&user_id).await?.name, "Retry:FlipFailed");
    assert_eq!(
        find_post_uri(&user_id, &post_id).await,
        Some(v1_post_uri_builder(user_id.clone(), post_id.clone()))
    );
    assert_eq!(
        find_post_details(&user_id, &post_id).await?.content,
        "Retry:FlipFailed:Post"
    );

    exec_single_row(queries::del::delete_post(&user_id, &post_id)).await?;
    exec_single_row(queries::del::delete_user(&user_id)).await?;
    Ok(())
}

/// The feed replays the batch while the queued flip waits: the replay flips the user and
/// indexes the profile. The retry then finds `v1`, runs as a profile fetch and leaves the
/// queue; the post is as the replay left it, migrated once.
#[tokio_shared_rt::test(shared)]
async fn test_a_flip_replayed_by_the_feed_and_retried_from_the_queue_converge() -> Result<()> {
    setup().await?;

    let user = random_pubky_id();
    let user_id = user.to_string();
    // No epoch on the node: the user reads as `v0`
    UserDetails::from_pubky(user).put_to_graph().await?;
    let (post, post_id, post_uri) = pubky_app_post(&user_id, "Retry:Flip:Replay:Post");
    let flip_uri = social_v1_profile_uri(&user_id);
    let profile = PubkySocialUser::new("Retry:Flip:Replay".to_string(), None, None, None, None);

    let store = new_in_memory_store();
    let files = tempfile::TempDir::new()?;
    let client = Arc::new(
        MockHomeserverClient::default()
            .with_lines(batch([format!("PUT {flip_uri}")]))
            .with_resource(&post_uri, serde_json::to_vec(&post)?)
            .with_resource(&flip_uri, serde_json::to_vec(&profile)?),
    );
    let handler = default_handler(client.clone(), &files, store.clone());
    retrieve_and_handle_event_line(&format!("PUT {post_uri}"), handler.clone()).await?;
    // The queue as a failed flip step left it
    store.put(&ready_put_entry(&flip_uri, 1)).await?;

    // The feed replays the batch
    let runner = primary_runner(client, handler.clone(), store.clone()).await?;
    runner
        .run_internal()
        .await
        .expect("the replayed flip must not fail the batch");
    assert_eq!(find_user_epoch(&user_id).await.as_deref(), Some("v1"));
    assert_eq!(find_user_details(&user_id).await?.name, "Retry:Flip:Replay");
    let v1_post_uri = v1_post_uri_builder(user_id.clone(), post_id.clone());
    assert_eq!(find_post_uri(&user_id, &post_id).await, Some(v1_post_uri));
    let post_after_replay = find_post_details(&user_id, &post_id).await?;
    assert!(
        store.get(&IndexKey::for_uri(&flip_uri)).await?.is_some(),
        "the replay leaves the queued entry in place"
    );

    // The queued flip is retried: a `v1` user's profile PUT
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(MAX_RETRIES, 50, 60, 3600, 60, 3600),
        handler,
        shutdown_rx,
    );
    processor
        .run_internal()
        .await
        .expect("the retried flip must not fail the batch");

    assert!(
        store.get(&IndexKey::for_uri(&flip_uri)).await?.is_none(),
        "the retried entry leaves the queue"
    );
    assert_eq!(find_user_epoch(&user_id).await.as_deref(), Some("v1"));
    assert_eq!(find_user_uri(&user_id).await, Some(flip_uri));
    assert_eq!(
        find_post_details(&user_id, &post_id).await?,
        post_after_replay,
        "the post is as the replay left it"
    );

    exec_single_row(queries::del::delete_post(&user_id, &post_id)).await?;
    exec_single_row(queries::del::delete_user(&user_id)).await?;
    Ok(())
}

// ============================================================================
// A `v1` user's profile PUT keeps the limit
// The rule covers only the flip: the same entry for a user already at `v1` is a
// profile edit, dead-lettered at `max_retries` like any other event.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_v1_user_profile_put_at_max_retries_with_5xx_is_dead_lettered() -> Result<()> {
    setup().await?;

    let user = random_pubky_id();
    let user_id = user.to_string();
    UserDetails::from_pubky(user).put_to_graph().await?;
    set_user_epoch(&user_id, "v1").await?;

    let event_uri = social_v1_profile_uri(&user_id);
    let store = new_in_memory_store();
    store.put(&ready_put_entry(&event_uri, MAX_RETRIES)).await?;

    let files = tempfile::TempDir::new()?;
    let handler = default_handler(
        MockHomeserverClient::default().with_resource_error(&event_uri, outage()),
        &files,
        store.clone(),
    );
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(MAX_RETRIES, 50, 60, 3600, 60, 3600),
        handler,
        shutdown_rx,
    );

    processor
        .run_internal()
        .await
        .expect("a dead-lettered entry must not fail the batch");

    assert!(
        store.get(&IndexKey::for_uri(&event_uri)).await?.is_none(),
        "a v1 user's profile edit must be dead-lettered at max_retries"
    );

    exec_single_row(queries::del::delete_user(&user_id)).await?;
    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// A flip with a `Generic` error keeps the limit
// Only a failed flip step lifts the limit: any other error on a `v0` user's
// profile PUT dead-letters it at `max_retries`.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_flip_at_max_retries_with_a_generic_error_is_dead_lettered() -> Result<()> {
    setup().await?;

    let user = random_pubky_id();
    let user_id = user.to_string();
    UserDetails::from_pubky(user).put_to_graph().await?;

    let event_uri = social_v1_profile_uri(&user_id);
    let store = new_in_memory_store();
    store.put(&ready_put_entry(&event_uri, MAX_RETRIES)).await?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(MAX_RETRIES, 50, 60, 3600, 60, 3600),
        create_mock_handler(
            Err(EventProcessorError::Generic("would repeat".to_string())),
            Some(&user_id),
        ),
        shutdown_rx,
    );

    processor
        .run_internal()
        .await
        .expect("a dead-lettered entry must not fail the batch");

    assert!(
        store.get(&IndexKey::for_uri(&event_uri)).await?.is_none(),
        "a flip with a Generic error must be dead-lettered at max_retries"
    );

    exec_single_row(queries::del::delete_user(&user_id)).await?;
    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// A `pubky.app` post PUT keeps the limit
// Unchanged: a transient fetch error on any other event still dead-letters it at
// `max_retries`.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_pubky_app_post_put_at_max_retries_with_5xx_is_dead_lettered() -> Result<()> {
    setup().await?;

    let user = random_pubky_id();
    let user_id = user.to_string();
    UserDetails::from_pubky(user).put_to_graph().await?;

    let event_uri = post_uri_builder(user_id.clone(), "0032SSN7Q4EVG".to_string());
    let store = new_in_memory_store();
    store.put(&ready_put_entry(&event_uri, MAX_RETRIES)).await?;

    let files = tempfile::TempDir::new()?;
    let handler = default_handler(
        MockHomeserverClient::default().with_resource_error(&event_uri, outage()),
        &files,
        store.clone(),
    );
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(MAX_RETRIES, 50, 60, 3600, 60, 3600),
        handler,
        shutdown_rx,
    );

    processor
        .run_internal()
        .await
        .expect("a dead-lettered entry must not fail the batch");

    assert!(
        store.get(&IndexKey::for_uri(&event_uri)).await?.is_none(),
        "a post PUT must still be dead-lettered at max_retries"
    );

    exec_single_row(queries::del::delete_user(&user_id)).await?;
    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// A graph error on the flip is unchanged
// A connection-class graph error outside the flip step, such as the gate's own
// read: rescheduled without counting, and the batch stops, so the entry behind it
// is not reached.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_graph_error_on_the_flip_is_rescheduled_without_counting_and_stops_the_batch(
) -> Result<()> {
    setup().await?;

    let user = random_pubky_id();
    let user_id = user.to_string();
    UserDetails::from_pubky(user).put_to_graph().await?;

    let profile_uri = social_v1_profile_uri(&user_id);
    let post_uri = post_uri_builder(user_id.clone(), "0032SSN7Q4EVH".to_string());
    let store = new_in_memory_store();
    let now = Utc::now().timestamp_millis();
    // The flip is due first, the post behind it
    store
        .put(&RetryEvent {
            next_retry_at: now - 2000,
            ..ready_put_entry(&profile_uri, MAX_RETRIES)
        })
        .await?;
    store.put(&ready_put_entry(&post_uri, 0)).await?;

    let handler = create_mock_handler(
        Err(EventProcessorError::GraphQueryFailed(
            true,
            "Database connection failed".to_string(),
        )),
        Some(&user_id),
    );
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(MAX_RETRIES, 50, 60, 3600, 60, 3600),
        handler.clone(),
        shutdown_rx,
    );

    let result = processor.run_internal().await;
    assert!(result.is_err(), "a graph error must stop the batch");
    assert_eq!(
        handler.get_handle_count(),
        1,
        "the post behind the flip must not be reached"
    );

    let rescheduled = store
        .get(&IndexKey::for_uri(&profile_uri))
        .await?
        .expect("the flip must be rescheduled, not removed");
    assert_eq!(
        rescheduled.retry_count, MAX_RETRIES,
        "a graph error does not count"
    );
    assert!(rescheduled.next_retry_at > now, "rescheduled with backoff");
    assert!(
        store.get(&IndexKey::for_uri(&post_uri)).await?.is_some(),
        "the post entry stays queued"
    );

    exec_single_row(queries::del::delete_user(&user_id)).await?;
    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// A `v0` user's `pubky.app` DELs are deferred while their flip waits in the queue
// The migrator writes the flip last; an old client may still delete the `pubky.app`
// copies after it. A flip whose step failed sits in the retry queue with the user
// still at `v0`; the DELs that follow would otherwise be handled and delete what the
// flip's internal migration is about to move. These tests drive the events through
// the primary runner and the retry processor with the real handler, one in-memory
// queue shared by the scheduler, the handler's gate and the processor.
// ============================================================================

/// Initial backoff of a transient error on the first attempt.
const TRANSIENT_MS: i64 = 10_000;
/// Initial backoff of a missing dependency on the first attempt: the dependency class shows
/// in an entry's due time.
const MISSING_DEP_MS: i64 = 60_000;
/// Attempts a missing dependency gets before it is dead-lettered.
const MAX_DEPENDENCY_RETRIES: u32 = 50;

/// The primary-homeserver runner: polls `client` once, handles the batch through `handler`
/// and queues what fails on `store`.
async fn primary_runner(
    client: Arc<MockHomeserverClient>,
    handler: Arc<dyn EventHandler>,
    store: Arc<dyn RetryStore>,
) -> Result<Arc<HsEventProcessor>> {
    let homeserver = Homeserver {
        id: random_pubky_id(),
        cursor: 0,
    };
    homeserver.put_to_index().await?;
    Ok(Arc::new(HsEventProcessor {
        homeserver,
        limit: 100,
        event_handler: handler,
        client,
        shutdown_rx: watch::channel(false).1,
        retry_scheduler: Arc::new(RetryScheduler::new(
            store,
            InitialBackoff {
                missing_dep_ms: MISSING_DEP_MS,
                transient_ms: TRANSIENT_MS,
            },
        )),
        hs_mapping_cache: Default::default(),
    }))
}

/// Closes `event_lines` into a batch the runner accepts: a cursor line past the stored 0.
fn batch(event_lines: impl IntoIterator<Item = String>) -> Vec<String> {
    event_lines
        .into_iter()
        .chain(["cursor: 1".to_string()])
        .collect()
}

/// The URI of `user_id`'s `pubky.app` profile.
fn pubky_app_profile_uri(user_id: &str) -> String {
    format!("pubky://{user_id}/pub/pubky.app/profile.json")
}

/// A `pubky.app` post of `user_id` and its URI, which the migrator's cleanup DELs name.
fn pubky_app_post(user_id: &str, content: &str) -> (PubkyAppPost, String, String) {
    let post = PubkyAppPost {
        content: content.to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
        lock: None,
    };
    let post_id = post.create_id();
    let uri = post_uri_builder(user_id.to_string(), post_id.clone());
    (post, post_id, uri)
}

/// A DEL entry of `event_uri`, ready for retry at `next_retry_at`.
fn del_entry(event_uri: &str, next_retry_at: i64) -> RetryEvent {
    RetryEvent {
        event_type: EventType::Del,
        next_retry_at,
        ..ready_put_entry(event_uri, 0)
    }
}

/// The flip's posts step fails on a row it can't read and the flip is queued; the DELs of
/// the user's post and `pubky.app` profile behind it are deferred: each is a missing
/// dependency naming the flip, queued with the dependency class, and the graph keeps the
/// post and the user.
#[tokio_shared_rt::test(shared)]
async fn test_pubky_app_dels_of_a_v0_user_are_deferred_while_their_flip_is_queued() -> Result<()> {
    setup().await?;

    let user = random_pubky_id();
    let user_id = user.to_string();
    // No epoch on the node: the user reads as `v0`
    UserDetails::from_pubky(user.clone()).put_to_graph().await?;

    let (post, post_id, post_uri) = pubky_app_post(&user_id, "Retry:Defer:Post");
    let flip_uri = social_v1_profile_uri(&user_id);
    let profile_uri = pubky_app_profile_uri(&user_id);
    let profile = PubkySocialUser::new("Retry:Defer".to_string(), None, None, None, None);

    // The user's post, indexed; then the migrator's run: the flip, whose posts step fails
    // on the post's row, and an old client's deletes that must wait for it
    let client = Arc::new(
        MockHomeserverClient::default()
            .with_lines(batch([
                format!("PUT {flip_uri}"),
                format!("DEL {post_uri}"),
                format!("DEL {profile_uri}"),
            ]))
            .with_resource(&post_uri, serde_json::to_vec(&post)?)
            .with_resource(&flip_uri, serde_json::to_vec(&profile)?),
    );
    let store = new_in_memory_store();
    let files = tempfile::TempDir::new()?;
    let handler = default_handler(client.clone(), &files, store.clone());
    retrieve_and_handle_event_line(&format!("PUT {post_uri}"), handler.clone()).await?;
    corrupt_post_indexed_at(&user_id, &post_id).await?;
    let runner = primary_runner(client, handler.clone(), store.clone()).await?;

    let now = Utc::now().timestamp_millis();
    runner
        .run_internal()
        .await
        .expect("a queued flip and deferred DELs must not fail the batch");

    let flip = store
        .get(&IndexKey::for_uri(&flip_uri))
        .await?
        .expect("the flip must be queued");
    assert_eq!(flip.event_type, EventType::Put);
    assert!(
        flip.next_retry_at < now + MISSING_DEP_MS,
        "the flip is queued with the transient class"
    );
    for uri in [&post_uri, &profile_uri] {
        let deferred = store
            .get(&IndexKey::for_uri(uri))
            .await?
            .unwrap_or_else(|| panic!("the DEL of {uri} must be queued"));
        assert_eq!(deferred.event_type, EventType::Del, "{uri}");
        assert!(
            deferred.next_retry_at >= now + MISSING_DEP_MS,
            "the DEL of {uri} is queued with the dependency class"
        );

        // What the runner queued: a missing dependency naming the flip
        let err = retrieve_and_handle_event_line(&format!("DEL {uri}"), handler.clone())
            .await
            .expect_err("a deferred DEL is an error, so the runner queues it");
        assert!(
            matches!(&err, EventProcessorError::MissingDependency { dependency } if *dependency == vec![flip_uri.clone()]),
            "{uri}: {err:?}"
        );
    }

    assert!(
        find_post_uri(&user_id, &post_id).await.is_some(),
        "the deferred DEL must leave the post node"
    );
    assert!(
        UserDetails::get_from_graph(&[user_id.as_str()]).await?[0].is_some(),
        "the deferred DEL must leave the user node"
    );
    assert_eq!(
        find_user_epoch(&user_id).await,
        None,
        "the user stays at v0 until the flip lands"
    );

    exec_single_row(queries::del::delete_post(&user_id, &post_id)).await?;
    exec_single_row(queries::del::delete_user(&user_id)).await?;
    Ok(())
}

/// The homeserver is back: the queued flip lands first, moving the user to `v1` and the
/// post to its `social/v1` address; the deferred DELs behind it are then ignored by the
/// gate and leave the queue, the post kept.
#[tokio_shared_rt::test(shared)]
async fn test_deferred_dels_are_ignored_and_leave_the_queue_once_the_flip_lands() -> Result<()> {
    setup().await?;

    let user = random_pubky_id();
    let user_id = user.to_string();
    UserDetails::from_pubky(user.clone()).put_to_graph().await?;

    let (post, post_id, post_uri) = pubky_app_post(&user_id, "Retry:Defer:Flipped");
    let flip_uri = social_v1_profile_uri(&user_id);
    let profile_uri = pubky_app_profile_uri(&user_id);
    let profile = PubkySocialUser::new("Retry:Defer:Flipped".to_string(), None, None, None, None);

    let store = new_in_memory_store();
    let files = tempfile::TempDir::new()?;
    let handler = default_handler(
        MockHomeserverClient::default()
            .with_resource(&post_uri, serde_json::to_vec(&post)?)
            .with_resource(&flip_uri, serde_json::to_vec(&profile)?),
        &files,
        store.clone(),
    );
    retrieve_and_handle_event_line(&format!("PUT {post_uri}"), handler.clone()).await?;

    // The queue as the outage left it: the flip due first, the deferred DELs behind it
    let now = Utc::now().timestamp_millis();
    store
        .put(&RetryEvent {
            next_retry_at: now - 2000,
            ..ready_put_entry(&flip_uri, 1)
        })
        .await?;
    store.put(&del_entry(&post_uri, now - 1000)).await?;
    store.put(&del_entry(&profile_uri, now - 1000)).await?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(MAX_RETRIES, 50, 60, 3600, 60, 3600),
        handler,
        shutdown_rx,
    );

    processor
        .run_internal()
        .await
        .expect("the flip and the ignored DELs must not fail the batch");

    assert_eq!(find_user_epoch(&user_id).await.as_deref(), Some("v1"));
    assert_eq!(
        find_post_uri(&user_id, &post_id).await.as_deref(),
        Some(v1_post_uri_builder(user_id.clone(), post_id.clone()).as_str()),
        "the internal migration moved the post to its v1 address"
    );
    assert!(
        PostDetails::get_from_graph(&user_id, &post_id)
            .await?
            .is_some(),
        "the retried DELs must leave the post in place"
    );
    for uri in [&flip_uri, &post_uri, &profile_uri] {
        assert!(
            store.get(&IndexKey::for_uri(uri)).await?.is_none(),
            "the entry of {uri} must leave the queue"
        );
    }

    exec_single_row(queries::del::delete_post(&user_id, &post_id)).await?;
    handlers::user::del(user).await?;
    let _ = shutdown_tx.send(true);
    Ok(())
}

/// A `v0` user with an indexed `pubky.app` post, their flip queued and not yet due, and the
/// post's DEL queued and due at `retry_count` attempts: the queue as an outage leaves it when
/// the retry processor picks the held DEL up before the flip. Returns the store, the handler,
/// the user's id and the post's id and URI; `files` must outlive the handler.
async fn held_del_queue(
    content: &str,
    retry_count: u32,
    files: &tempfile::TempDir,
) -> Result<(
    Arc<dyn RetryStore>,
    Arc<dyn EventHandler>,
    String,
    String,
    String,
)> {
    let user = random_pubky_id();
    let user_id = user.to_string();
    UserDetails::from_pubky(user).put_to_graph().await?;
    let (post, post_id, post_uri) = pubky_app_post(&user_id, content);
    let store = new_in_memory_store();
    let handler = default_handler(
        MockHomeserverClient::default().with_resource(&post_uri, serde_json::to_vec(&post)?),
        files,
        store.clone(),
    );
    retrieve_and_handle_event_line(&format!("PUT {post_uri}"), handler.clone()).await?;

    store
        .put(&RetryEvent {
            next_retry_at: i64::MAX,
            ..ready_put_entry(&social_v1_profile_uri(&user_id), 1)
        })
        .await?;
    store
        .put(&RetryEvent {
            retry_count,
            ..del_entry(&post_uri, Utc::now().timestamp_millis() - 1000)
        })
        .await?;
    Ok((store, handler, user_id, post_id, post_uri))
}

/// What a held DEL must leave alone, whether it was rescheduled or dead-lettered: the flip
/// still queued, the post still readable, the user still at `v0`.
async fn assert_held_del_left_the_post(
    store: &dyn RetryStore,
    user_id: &str,
    post_id: &str,
) -> Result<()> {
    assert!(
        store
            .get(&IndexKey::for_uri(&social_v1_profile_uri(user_id)))
            .await?
            .is_some(),
        "the flip stays queued"
    );
    assert!(
        PostDetails::get_from_graph(user_id, post_id)
            .await?
            .is_some(),
        "the held DEL must leave the post readable"
    );
    assert_eq!(
        find_user_epoch(user_id).await,
        None,
        "the user stays at v0 until the flip lands"
    );
    Ok(())
}

/// The retry processor picks a held DEL up while the flip still waits: the gate holds it
/// again and it is rescheduled with the dependency class, its count raised; the post and
/// the user's epoch are untouched.
#[tokio_shared_rt::test(shared)]
async fn test_deferred_del_retried_while_the_flip_waits_is_rescheduled_with_the_post_kept(
) -> Result<()> {
    setup().await?;

    let files = tempfile::TempDir::new()?;
    let (store, handler, user_id, post_id, post_uri) =
        held_del_queue("Retry:Defer:Held", 0, &files).await?;

    let now = Utc::now().timestamp_millis();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    // A transient backoff of 10 s, a dependency one of 60 s: the class shows in the due time
    let processor = build_processor(
        store.clone(),
        create_test_config(MAX_RETRIES, MAX_DEPENDENCY_RETRIES, 10, 3600, 60, 3600),
        handler,
        shutdown_rx,
    );

    processor
        .run_internal()
        .await
        .expect("a DEL held again must not fail the batch");

    let held = store
        .get(&IndexKey::for_uri(&post_uri))
        .await?
        .expect("the held DEL must stay queued");
    assert_eq!(held.retry_count, 1, "the retry counts");
    assert!(
        held.next_retry_at >= now + MISSING_DEP_MS,
        "the DEL is rescheduled with the dependency class"
    );
    assert_held_del_left_the_post(store.as_ref(), &user_id, &post_id).await?;

    exec_single_row(queries::del::delete_post(&user_id, &post_id)).await?;
    exec_single_row(queries::del::delete_user(&user_id)).await?;
    let _ = shutdown_tx.send(true);
    Ok(())
}

/// A held DEL at the dependency limit while the flip still waits is dead-lettered: it
/// leaves the queue and the post stays in the graph.
#[tokio_shared_rt::test(shared)]
async fn test_deferred_del_at_the_dependency_limit_is_dead_lettered_with_the_post_kept(
) -> Result<()> {
    setup().await?;

    let files = tempfile::TempDir::new()?;
    let (store, handler, user_id, post_id, post_uri) =
        held_del_queue("Retry:Defer:HeldLimit", MAX_DEPENDENCY_RETRIES, &files).await?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(MAX_RETRIES, MAX_DEPENDENCY_RETRIES, 10, 3600, 60, 3600),
        handler,
        shutdown_rx,
    );

    processor
        .run_internal()
        .await
        .expect("a dead-lettered DEL must not fail the batch");

    assert!(
        store.get(&IndexKey::for_uri(&post_uri)).await?.is_none(),
        "the held DEL leaves the queue at the dependency limit"
    );
    assert_held_del_left_the_post(store.as_ref(), &user_id, &post_id).await?;

    exec_single_row(queries::del::delete_post(&user_id, &post_id)).await?;
    exec_single_row(queries::del::delete_user(&user_id)).await?;
    let _ = shutdown_tx.send(true);
    Ok(())
}

/// With no flip queued, a `v0` user's `pubky.app` post DEL is handled as today: the post
/// is gone and nothing is queued.
#[tokio_shared_rt::test(shared)]
async fn test_pubky_app_del_of_a_v0_user_without_a_queued_flip_is_handled() -> Result<()> {
    setup().await?;

    let user = random_pubky_id();
    let user_id = user.to_string();
    UserDetails::from_pubky(user.clone()).put_to_graph().await?;

    let (post, post_id, post_uri) = pubky_app_post(&user_id, "Retry:Defer:NoFlip");
    let client = Arc::new(
        MockHomeserverClient::default()
            .with_lines(batch([
                format!("PUT {post_uri}"),
                format!("DEL {post_uri}"),
            ]))
            .with_resource(&post_uri, serde_json::to_vec(&post)?),
    );
    let store = new_in_memory_store();
    let files = tempfile::TempDir::new()?;
    let handler = default_handler(client.clone(), &files, store.clone());
    let runner = primary_runner(client, handler, store.clone()).await?;

    runner
        .run_internal()
        .await
        .expect("a handled DEL must not fail the batch");

    assert!(
        PostDetails::get_from_graph(&user_id, &post_id)
            .await?
            .is_none(),
        "the DEL must delete the post"
    );
    assert!(
        store.fetch_ready(i64::MAX, None).await?.is_empty(),
        "nothing is queued"
    );

    exec_single_row(queries::del::delete_user(&user_id)).await?;
    Ok(())
}

// ============================================================================
// A failed flip step is queued without limit while the user is `v0`
// `FlipFailed` wraps whatever failed inside the flip step, and its retry class is
// the variant's, not the source's: enqueued on the first attempt with the transient
// backoff while the batch goes on, counted with `max_retries`, and rescheduled past
// that limit for as long as the user is still `v0` in the graph. The flip step
// produces it (the tests through the real handler above); these return it from a
// stub handler, through the primary runner and the retry processor with the
// in-memory queue, to cover every class of cause. The `v1` user's case,
// dead-lettered at `warn`, is in the processor's own tests, where its log line can
// be read.
// ============================================================================

/// The step's failure, wrapped as the flip returns it.
fn flip_failed(source: EventProcessorError) -> EventProcessorError {
    EventProcessorError::FlipFailed {
        step: "posts",
        source: Box::new(source),
    }
}

/// What a flip step can fail with, one per retry class of its own: a connection-class
/// graph error (a batch stop), a deserialization-class one (counted), and a stalled keyset
/// scan (never retried). Wrapped, each takes the flip's class instead.
fn flip_step_failures() -> [EventProcessorError; 3] {
    [
        EventProcessorError::GraphQueryFailed(true, "connection refused".to_string()),
        EventProcessorError::GraphQueryFailed(false, "Deserialization failed".to_string()),
        EventProcessorError::DataAnomaly("keyset scan cannot advance".to_string()),
    ]
}

/// A `pubky.app` post URI of another user: the event behind the flip in a batch.
fn another_users_post_uri() -> String {
    post_uri_builder(random_pubky_id().to_string(), "0032SSN7Q4EVG".to_string())
}

/// The runner's first attempt: the flip step fails, the flip is queued with the transient
/// class and the next event is handled, whichever of the three sources failed the step.
#[tokio_shared_rt::test(shared)]
async fn test_a_failed_flip_step_is_queued_on_the_first_attempt_and_the_batch_continues(
) -> Result<()> {
    setup().await?;

    for source in flip_step_failures() {
        let flip_uri = social_v1_profile_uri(random_pubky_id().as_ref());
        let post_uri = another_users_post_uri();
        let client = Arc::new(MockHomeserverClient::default().with_lines(batch([
            format!("PUT {flip_uri}"),
            format!("PUT {post_uri}"),
        ])));
        let store = new_in_memory_store();
        let handler = create_mock_handler(Err(flip_failed(source.clone())), Some(&flip_uri));
        let runner = primary_runner(client, handler.clone(), store.clone()).await?;

        let now = Utc::now().timestamp_millis();
        runner.run_internal().await.unwrap_or_else(|e| {
            panic!("{source:?}: a failed flip step must not fail the batch: {e}")
        });

        assert_eq!(
            handler.get_handled_uris(),
            vec![flip_uri.clone(), post_uri.clone()],
            "{source:?}: the event behind the flip is handled"
        );
        let queued = store
            .get(&IndexKey::for_uri(&flip_uri))
            .await?
            .unwrap_or_else(|| panic!("{source:?}: the flip must be queued"));
        assert_eq!(queued.event_type, EventType::Put, "{source:?}");
        assert_eq!(queued.retry_count, 0, "{source:?}");
        assert!(
            (now + TRANSIENT_MS..now + MISSING_DEP_MS).contains(&queued.next_retry_at),
            "{source:?}: the flip is queued with the transient class"
        );
        assert!(
            store.get(&IndexKey::for_uri(&post_uri)).await?.is_none(),
            "{source:?}: the handled post is not queued"
        );
    }
    Ok(())
}

/// The retry processor below the limit: the flip step fails again, the entry is rescheduled
/// with the transient backoff and its count raised, and the entry behind it is handled.
#[tokio_shared_rt::test(shared)]
async fn test_a_failed_flip_step_retried_below_the_limit_is_rescheduled_and_the_batch_continues(
) -> Result<()> {
    setup().await?;

    for source in flip_step_failures() {
        let flip_uri = social_v1_profile_uri(random_pubky_id().as_ref());
        let post_uri = another_users_post_uri();
        let store = new_in_memory_store();
        let now = Utc::now().timestamp_millis();
        // The flip is due first, the post behind it
        store
            .put(&RetryEvent {
                next_retry_at: now - 2000,
                ..ready_put_entry(&flip_uri, 0)
            })
            .await?;
        store.put(&ready_put_entry(&post_uri, 0)).await?;

        let handler = create_mock_handler(Err(flip_failed(source.clone())), Some(&flip_uri));
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        let processor = build_processor(
            store.clone(),
            // Transient: 60 s doubling to 1 h; dependency: 600 s doubling to 2 h
            create_test_config(MAX_RETRIES, 50, 60, 3600, 600, 7200),
            handler.clone(),
            shutdown_rx,
        );

        processor.run_internal().await.unwrap_or_else(|e| {
            panic!("{source:?}: a failed flip step must not fail the batch: {e}")
        });

        assert_eq!(
            handler.get_handled_uris(),
            vec![flip_uri.clone(), post_uri.clone()],
            "{source:?}: the entry behind the flip is handled"
        );
        let rescheduled = store
            .get(&IndexKey::for_uri(&flip_uri))
            .await?
            .unwrap_or_else(|| panic!("{source:?}: the flip must be rescheduled"));
        assert_eq!(rescheduled.retry_count, 1, "{source:?}: the attempt counts");
        assert!(
            (now + 60_000..now + 600_000).contains(&rescheduled.next_retry_at),
            "{source:?}: the flip takes the transient backoff, not the dependency one"
        );
        assert!(
            store.get(&IndexKey::for_uri(&post_uri)).await?.is_none(),
            "{source:?}: the handled post leaves the queue"
        );
    }
    Ok(())
}

/// At `max_retries`, a flip whose step keeps failing is rescheduled with its count raised, not
/// removed, while the user is still `v0`: the attempts count, the limit does not apply.
#[tokio_shared_rt::test(shared)]
async fn test_a_failed_flip_step_at_max_retries_is_rescheduled_while_the_user_is_v0() -> Result<()>
{
    setup().await?;

    let user = random_pubky_id();
    let user_id = user.to_string();
    // No epoch on the node: the user reads as `v0`
    UserDetails::from_pubky(user).put_to_graph().await?;
    let flip_uri = social_v1_profile_uri(&user_id);

    for source in flip_step_failures() {
        let store = new_in_memory_store();
        let now = Utc::now().timestamp_millis();
        store.put(&ready_put_entry(&flip_uri, MAX_RETRIES)).await?;

        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        let processor = build_processor(
            store.clone(),
            create_test_config(MAX_RETRIES, 50, 60, 3600, 600, 7200),
            create_mock_handler(Err(flip_failed(source.clone())), Some(&flip_uri)),
            shutdown_rx,
        );

        processor.run_internal().await.unwrap_or_else(|e| {
            panic!("{source:?}: a failed flip step must not fail the batch: {e}")
        });

        let rescheduled = store
            .get(&IndexKey::for_uri(&flip_uri))
            .await?
            .unwrap_or_else(|| {
                panic!(
                    "{source:?}: the flip must be rescheduled past max_retries, not dead-lettered"
                )
            });
        assert_eq!(
            rescheduled.retry_count,
            MAX_RETRIES + 1,
            "{source:?}: the attempt still counts"
        );
        assert!(
            (now + 3_600_000..now + 3_600_000 + 60_000).contains(&rescheduled.next_retry_at),
            "{source:?}: the flip is rescheduled at the transient ceiling"
        );
        assert_eq!(
            find_user_epoch(&user_id).await,
            None,
            "{source:?}: the user stays at v0 until the step succeeds"
        );
    }

    exec_single_row(queries::del::delete_user(&user_id)).await?;
    Ok(())
}

/// A failed graph read inside the limit check takes the handler's error's place: the entry is
/// rescheduled without counting, as for any graph failure, and the batch stops. The stack is
/// never set up here, so the gate's read fails on the uninitialised connection; nextest runs
/// each test in its own process, so no other test initialises it first.
#[tokio_shared_rt::test(shared)]
async fn test_a_failed_graph_read_in_the_limit_check_keeps_the_flip_entry() -> Result<()> {
    let flip_uri = social_v1_profile_uri(random_pubky_id().as_ref());
    let post_uri = another_users_post_uri();
    let store = new_in_memory_store();
    let now = Utc::now().timestamp_millis();
    // The flip is due first, the post behind it
    store
        .put(&RetryEvent {
            next_retry_at: now - 2000,
            ..ready_put_entry(&flip_uri, MAX_RETRIES)
        })
        .await?;
    store.put(&ready_put_entry(&post_uri, 0)).await?;

    let handler = create_mock_handler(
        Err(flip_failed(EventProcessorError::DataAnomaly(
            "keyset scan cannot advance".to_string(),
        ))),
        Some(&flip_uri),
    );
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let processor = build_processor(
        store.clone(),
        create_test_config(MAX_RETRIES, 50, 60, 3600, 600, 7200),
        handler.clone(),
        shutdown_rx,
    );

    let err = processor
        .run_internal()
        .await
        .expect_err("a failed graph read must stop the batch");
    assert!(
        matches!(err, EventProcessorError::GraphQueryFailed(true, _)),
        "classified as the graph error it is: {err:?}"
    );
    assert_eq!(
        handler.get_handled_uris(),
        vec![flip_uri.clone()],
        "the post behind the flip must not be reached"
    );

    let kept = store
        .get(&IndexKey::for_uri(&flip_uri))
        .await?
        .expect("the flip must be kept, not dead-lettered");
    assert_eq!(
        kept.retry_count, MAX_RETRIES,
        "a graph error does not count"
    );
    assert!(kept.next_retry_at > now, "rescheduled with backoff");
    assert!(
        store.get(&IndexKey::for_uri(&post_uri)).await?.is_some(),
        "the post entry stays queued"
    );
    Ok(())
}
