use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use nexus_common::db::{exec_single_row, queries, PubkyClientError, PubkyClientResult};
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::user::{Epoch, UserDetails};
use nexus_common::utils::test_utils::{random_pk, random_pubky_id};
use nexus_watcher::errors::EventProcessorError;
use nexus_watcher::events::retry::{IndexKey, InitialBackoff, RetryScheduler, RetryStore};
use nexus_watcher::events::EventHandler;
use nexus_watcher::service::user_hs_resolver::PkdnsHomeserverResolver;
use nexus_watcher::service::HsEventProcessor;
use pubky::PublicKey;
use pubky_app_specs::{post_uri_builder, PubkyId};
use tokio::sync::watch;

use crate::service::utils::common::create_mock_handler;
use crate::service::utils::{new_in_memory_store, setup};

const TEST_HS_ID: &str = "1hb71xx9km3f4pw5izsy1gn19ff1uuuqonw4mcygzobwkryujoiy";

/// Cursor line closing the batches of tests that are not about cursor handling.
///
/// Any value above the 0 that [`build_processor`] sits at works, and using a
/// single shared one keeps it valid however many of these tests persist it for
/// [`TEST_HS_ID`]: re-persisting the same cursor is not a rewind.
const TEST_NEXT_CURSOR: &str = "cursor: 1";

/// Returns a fresh random user id (z32 public key) that has no graph state yet.
fn random_user_id() -> String {
    random_pk().to_z32()
}

/// Closes `event_lines` into a well-formed batch, the way a homeserver ends a
/// non-empty response.
///
/// Tests that are not about cursor handling still need this: a batch carrying
/// events but no cursor line is rejected as stalled, since nothing in it could
/// move the checkpoint past the events it delivers.
fn batch(event_lines: impl IntoIterator<Item = String>) -> Vec<String> {
    event_lines
        .into_iter()
        .chain([TEST_NEXT_CURSOR.to_string()])
        .collect()
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
        deleted: false,
        epoch: Epoch::V0,
    };
    exec_single_row(queries::put::create_user(&user, None).expect("create_user query"))
        .await
        .expect("create user node");

    if let Some(hs_id) = hs_id {
        exec_single_row(queries::put::set_user_homeserver(user_id, hs_id))
            .await
            .expect("link user to homeserver");
    }
}

/// What every user without a `HOSTED_BY` edge publishes, as seen by [`PublishedHs`].
#[derive(Clone)]
enum Published {
    Homeserver(String),
    Nothing,
    LookupFails,
}

/// Resolver stub answering every lookup the same way, so the processor's homeserver
/// lookup for users without a `HOSTED_BY` edge runs without touching the DHT.
struct PublishedHs(Published);

#[async_trait::async_trait]
impl PkdnsHomeserverResolver for PublishedHs {
    async fn resolve_homeserver(&self, _user_pk: &PublicKey) -> PubkyClientResult<Option<PubkyId>> {
        match &self.0 {
            Published::Homeserver(hs_id) => Ok(Some(
                PubkyId::try_from(hs_id.as_str()).expect("Valid homeserver Pubky ID"),
            )),
            Published::Nothing => Ok(None),
            Published::LookupFails => Err(PubkyClientError::RequestFailed {
                message: "dht unreachable".into(),
            }),
        }
    }
}

fn test_retry_scheduler(store: Arc<dyn RetryStore>) -> Arc<RetryScheduler> {
    Arc::new(RetryScheduler::new(
        store,
        InitialBackoff {
            missing_dep_ms: 60_000,
            transient_ms: 10_000,
        },
    ))
}

/// Assemble an [`HsEventProcessor`] for tests. Tests bypass `poll_events` by
/// calling `process_event_lines` directly with constructed event lines. Users
/// without a `HOSTED_BY` edge publish this processor's homeserver.
fn build_processor(
    store: Arc<dyn RetryStore>,
    event_handler: Arc<dyn EventHandler>,
    shutdown_rx: watch::Receiver<bool>,
) -> Arc<HsEventProcessor> {
    build_processor_with(
        Published::Homeserver(TEST_HS_ID.to_string()),
        store,
        event_handler,
        shutdown_rx,
    )
}

/// [`build_processor`], with users without a `HOSTED_BY` edge publishing `published`.
fn build_processor_with(
    published: Published,
    store: Arc<dyn RetryStore>,
    event_handler: Arc<dyn EventHandler>,
    shutdown_rx: watch::Receiver<bool>,
) -> Arc<HsEventProcessor> {
    let hs_id = PubkyId::try_from(TEST_HS_ID).expect("Valid test Pubky ID");

    Arc::new(HsEventProcessor {
        homeserver: Homeserver::new(hs_id),
        limit: 100,
        event_handler,
        shutdown_rx,
        retry_scheduler: test_retry_scheduler(store),
        hs_mapping_cache: Default::default(),
        hs_resolver: Arc::new(PublishedHs(published)),
    })
}

/// Assembles an [`HsEventProcessor`] sitting at `cursor` on a fresh random
/// homeserver, with that same cursor seeded in Redis — mirroring a real
/// processor, which is built by reading the stored cursor and then requesting
/// events from exactly that position.
///
/// A random homeserver id keeps the stored cursor from leaking between tests.
/// Users without a `HOSTED_BY` edge publish that homeserver.
async fn build_processor_at_cursor(
    cursor: u64,
    store: Arc<dyn RetryStore>,
    event_handler: Arc<dyn EventHandler>,
    shutdown_rx: watch::Receiver<bool>,
) -> Result<Arc<HsEventProcessor>> {
    build_processor_at_cursor_with(cursor, None, store, event_handler, shutdown_rx).await
}

/// [`build_processor_at_cursor`], with users without a `HOSTED_BY` edge publishing
/// `published` (`None`: the processor's own homeserver).
async fn build_processor_at_cursor_with(
    cursor: u64,
    published: Option<Published>,
    store: Arc<dyn RetryStore>,
    event_handler: Arc<dyn EventHandler>,
    shutdown_rx: watch::Receiver<bool>,
) -> Result<Arc<HsEventProcessor>> {
    let hs_id = random_pubky_id();
    let published = published.unwrap_or_else(|| Published::Homeserver(hs_id.to_string()));
    let homeserver = Homeserver { id: hs_id, cursor };
    homeserver.put_to_index().await?;

    Ok(Arc::new(HsEventProcessor {
        homeserver,
        limit: 100,
        event_handler,
        shutdown_rx,
        retry_scheduler: test_retry_scheduler(store),
        hs_mapping_cache: Default::default(),
        hs_resolver: Arc::new(PublishedHs(published)),
    }))
}

/// Reads a homeserver's currently stored cursor.
async fn stored_cursor(hs_id: &str) -> Result<u64> {
    Ok(Homeserver::get_from_index(hs_id)
        .await?
        .expect("Homeserver must be present in the index")
        .cursor)
}

// ============================================================================
// Batch continues after a single event fails
// A retryable application error on one event must not halt the batch — later
// events still need to be handed to the event handler.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_batch_continues_after_single_failure() -> Result<()> {
    setup().await?;

    // Fresh user with no HOSTED_BY edge, publishing this homeserver, so the guard processes its events.
    let user_id = random_user_id();
    let first_post_id = "failone";
    let second_post_id = "failtwo";
    let first_uri = post_uri_builder(user_id.clone(), first_post_id.to_string());
    let second_uri = post_uri_builder(user_id, second_post_id.to_string());

    let lines = batch([format!("PUT {first_uri}"), format!("PUT {second_uri}")]);

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
        store.get(&IndexKey::for_uri(&first_uri)).await?.is_some(),
        "First event must be queued for retry"
    );
    assert!(
        store.get(&IndexKey::for_uri(&second_uri)).await?.is_some(),
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

    // Fresh user with no HOSTED_BY edge, publishing this homeserver, so the guard processes its events.
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
        .process_event_lines(batch([format!("PUT {uri}")]))
        .await?;

    let retry_event = store
        .get(&IndexKey::for_uri(&uri))
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
    let other_hs_id = random_pubky_id().to_string();
    create_user_hosted_on(&user_id, Some(&other_hs_id)).await;

    let uri = post_uri_builder(user_id.clone(), "skipme".to_string());

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    let processor = build_processor(store.clone(), handler.clone(), shutdown_rx);

    processor
        .process_event_lines(batch([format!("PUT {uri}")]))
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
        .process_event_lines(batch([format!("PUT {uri}")]))
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
        .process_event_lines(batch([format!("PUT {uri}")]))
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
// HOSTED_BY guard: users with no HOSTED_BY edge
// A missing edge is the common case (e.g. before the resolver has run). The
// homeserver the user publishes decides instead: only this homeserver's events
// for the user are processed.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_processes_event_when_user_without_edge_publishes_this_homeserver() -> Result<()> {
    setup().await?;

    // Fresh user id with no graph state: no User node, no HOSTED_BY edge.
    let user_id = random_user_id();
    let uri = post_uri_builder(user_id, "noedge".to_string());

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    let processor = build_processor(store.clone(), handler.clone(), shutdown_rx);

    processor
        .process_event_lines(batch([format!("PUT {uri}"), format!("DEL {uri}")]))
        .await?;

    assert_eq!(
        handler.get_handle_count(),
        2,
        "Events from a user without a HOSTED_BY edge who publishes this homeserver must be processed"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_skips_event_when_user_without_edge_publishes_another_homeserver() -> Result<()> {
    setup().await?;

    let uri = post_uri_builder(random_user_id(), "elsewhere".to_string());

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    let processor = build_processor_with(
        Published::Homeserver(random_pubky_id().to_string()),
        store.clone(),
        handler.clone(),
        shutdown_rx,
    );

    processor
        .process_event_lines(batch([format!("PUT {uri}"), format!("DEL {uri}")]))
        .await?;

    assert_eq!(
        handler.get_handle_count(),
        0,
        "Events from a user who publishes another homeserver must be skipped"
    );
    assert!(
        store.get(&IndexKey::for_uri(&uri)).await?.is_none(),
        "A skipped event must not be queued for retry"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_skips_event_when_user_without_edge_publishes_no_homeserver() -> Result<()> {
    setup().await?;

    let uri = post_uri_builder(random_user_id(), "unpublished".to_string());

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    let processor = build_processor_with(
        Published::Nothing,
        store.clone(),
        handler.clone(),
        shutdown_rx,
    );

    processor
        .process_event_lines(batch([format!("PUT {uri}"), format!("DEL {uri}")]))
        .await?;

    assert_eq!(
        handler.get_handle_count(),
        0,
        "Events from a user who publishes no homeserver must be skipped"
    );
    assert!(
        store.get(&IndexKey::for_uri(&uri)).await?.is_none(),
        "A skipped event must not be queued for retry"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// HOSTED_BY guard: a failed homeserver lookup stops the batch
// Without an answer the event can neither be processed nor queued (retries go
// straight to the handler), so the batch stops and the cursor stays put.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_failed_homeserver_lookup_stops_batch_and_keeps_cursor() -> Result<()> {
    setup().await?;

    let uri = post_uri_builder(random_user_id(), "lookupfails".to_string());

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    let processor = build_processor_at_cursor_with(
        10,
        Some(Published::LookupFails),
        store.clone(),
        handler.clone(),
        shutdown_rx,
    )
    .await?;

    let result = processor
        .process_event_lines(vec![format!("DEL {uri}"), "cursor: 11".to_string()])
        .await;

    assert!(
        matches!(result, Err(EventProcessorError::UserHsLookupFailed(_))),
        "A failed homeserver lookup must stop the batch, got {result:?}"
    );
    assert_eq!(
        handler.get_handle_count(),
        0,
        "The event must not be processed"
    );
    assert!(
        store.get(&IndexKey::for_uri(&uri)).await?.is_none(),
        "The event must not be queued for retry"
    );
    assert_eq!(
        stored_cursor(&processor.homeserver.id).await?,
        10,
        "The cursor must not advance past the event"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// HOSTED_BY guard: the mapping is cached for the processor's lifetime
// The first event resolves the user's mapping and caches it; later events reuse
// the cached decision even if the underlying graph state changes afterwards.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_user_homeserver_mapping_is_cached_across_events() -> Result<()> {
    setup().await?;

    // User starts with no HOSTED_BY edge and publishes this homeserver, so the first
    // event is processed and the resulting mapping is cached.
    let user_id = random_user_id();
    let first_uri = post_uri_builder(user_id.clone(), "cacheone".to_string());
    let second_uri = post_uri_builder(user_id.clone(), "cachetwo".to_string());

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    let processor = build_processor(store.clone(), handler.clone(), shutdown_rx);

    processor
        .process_event_lines(batch([format!("PUT {first_uri}")]))
        .await?;

    // Map the user to a *different* homeserver. A fresh lookup would now skip,
    // but the cached mapping must keep this user's events flowing.
    let other_hs_id = random_pubky_id().to_string();
    create_user_hosted_on(&user_id, Some(&other_hs_id)).await;

    processor
        .process_event_lines(batch([format!("PUT {second_uri}")]))
        .await?;

    assert_eq!(
        handler.get_handle_count(),
        2,
        "Second event must reuse the cached mapping and still be processed despite the new HOSTED_BY edge"
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

    // Fresh user with no HOSTED_BY edge, publishing this homeserver, so the guard processes its events.
    let user_id = random_user_id();
    let first_post_id = "infraone";
    let second_post_id = "infratwo";
    let first_uri = post_uri_builder(user_id.clone(), first_post_id.to_string());
    let second_uri = post_uri_builder(user_id, second_post_id.to_string());

    let lines = batch([format!("PUT {first_uri}"), format!("PUT {second_uri}")]);

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
        store.get(&IndexKey::for_uri(&first_uri)).await?.is_none(),
        "Should-not-retry-now errors must not be queued for retry"
    );
    assert!(
        store.get(&IndexKey::for_uri(&second_uri)).await?.is_none(),
        "Second event must not be queued — batch should have stopped at the first failure"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// A batch carrying events must advance the cursor
// A response fetched at cursor 10 that carries events but closes at cursor 10
// would be re-requested and re-processed on every subsequent poll. The batch is
// skipped before any handler runs, and the cursor is left where it was.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_non_advancing_cursor_skips_batch_with_events() -> Result<()> {
    setup().await?;

    let uri = post_uri_builder(random_user_id(), "stalled".to_string());

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    let processor =
        build_processor_at_cursor(10, store.clone(), handler.clone(), shutdown_rx).await?;

    processor
        .process_event_lines(vec![format!("PUT {uri}"), "cursor: 10".to_string()])
        .await?;

    assert_eq!(
        handler.get_handle_count(),
        0,
        "Batch must be skipped before any handler runs — the cursor could never record the work"
    );
    assert_eq!(
        stored_cursor(&processor.homeserver.id).await?,
        10,
        "A non-advancing cursor must not be persisted"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// A batch carrying events but no cursor line is skipped
// Nothing in such a batch can move the checkpoint past the events it delivers,
// so the next poll would re-fetch and re-process it — the same stall as a
// non-advancing cursor, reached by omission rather than repetition.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_batch_without_cursor_line_is_skipped() -> Result<()> {
    setup().await?;

    let uri = post_uri_builder(random_user_id(), "nocursor".to_string());

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    let processor =
        build_processor_at_cursor(10, store.clone(), handler.clone(), shutdown_rx).await?;

    processor
        .process_event_lines(vec![format!("PUT {uri}")])
        .await?;

    assert_eq!(
        handler.get_handle_count(),
        0,
        "A batch with events but no cursor line must be skipped before any handler runs"
    );
    assert_eq!(
        stored_cursor(&processor.homeserver.id).await?,
        10,
        "A batch with no cursor line must leave the cursor untouched"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// An idle poll may repeat the cursor
// A response with no event lines legitimately closes at the cursor it was
// fetched with; that is the homeserver saying "nothing new", not a stall.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_idle_batch_may_repeat_cursor() -> Result<()> {
    setup().await?;

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    let processor =
        build_processor_at_cursor(10, store.clone(), handler.clone(), shutdown_rx).await?;

    processor
        .process_event_lines(vec!["cursor: 10".to_string()])
        .await?;

    assert_eq!(
        stored_cursor(&processor.homeserver.id).await?,
        10,
        "An idle poll repeating the cursor must be accepted and leave it in place"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// An idle poll may not advance the cursor
// Without event lines, the response cannot prove the watcher received the
// events between the requested and returned cursors. Advancing would skip them.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_idle_batch_may_not_advance_cursor() -> Result<()> {
    setup().await?;

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    let processor =
        build_processor_at_cursor(10, store.clone(), handler.clone(), shutdown_rx).await?;

    processor
        .process_event_lines(vec!["cursor: 20".to_string()])
        .await?;

    assert_eq!(
        stored_cursor(&processor.homeserver.id).await?,
        10,
        "An idle poll must not advance past events it did not deliver"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// An advancing cursor is persisted once the batch has been processed
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_advancing_cursor_is_persisted_after_batch() -> Result<()> {
    setup().await?;

    let uri = post_uri_builder(random_user_id(), "advance".to_string());

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    let processor =
        build_processor_at_cursor(10, store.clone(), handler.clone(), shutdown_rx).await?;

    processor
        .process_event_lines(vec![format!("PUT {uri}"), "cursor: 11".to_string()])
        .await?;

    assert_eq!(
        handler.get_handle_count(),
        1,
        "Event must be handled when the batch closes with an advancing cursor"
    );
    assert_eq!(
        stored_cursor(&processor.homeserver.id).await?,
        11,
        "An advancing cursor must be persisted after the batch"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// A rewinding or unparseable cursor skips the batch before handlers run
// Validating up front means bad homeserver input costs no handler work, since
// the cursor stays put and the next poll re-fetches the same batch anyway.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_rewinding_cursor_skips_batch_before_handlers() -> Result<()> {
    setup().await?;

    let uri = post_uri_builder(random_user_id(), "rewind".to_string());

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    let processor =
        build_processor_at_cursor(10, store.clone(), handler.clone(), shutdown_rx).await?;

    processor
        .process_event_lines(vec![format!("PUT {uri}"), "cursor: 5".to_string()])
        .await?;

    assert_eq!(
        handler.get_handle_count(),
        0,
        "A rewinding cursor must skip the batch before any handler runs"
    );
    assert_eq!(
        stored_cursor(&processor.homeserver.id).await?,
        10,
        "A rewinding cursor must not be persisted"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_unparseable_cursor_skips_batch_before_handlers() -> Result<()> {
    setup().await?;

    let uri = post_uri_builder(random_user_id(), "garbage".to_string());

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(Ok(()), None);
    let processor =
        build_processor_at_cursor(10, store.clone(), handler.clone(), shutdown_rx).await?;

    processor
        .process_event_lines(vec![
            format!("PUT {uri}"),
            "cursor: not-a-number".to_string(),
        ])
        .await?;

    assert_eq!(
        handler.get_handle_count(),
        0,
        "An unparseable cursor must skip the batch before any handler runs"
    );
    assert_eq!(
        stored_cursor(&processor.homeserver.id).await?,
        10,
        "An unparseable cursor must not be persisted"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}

// ============================================================================
// The cursor is persisted only after the whole batch succeeds
// A should-not-retry-now error stops the batch partway, so the valid cursor
// closing it must not be applied — the next run has to replay from the same
// position.
// ============================================================================

#[tokio_shared_rt::test(shared)]
async fn test_cursor_not_persisted_when_batch_stops() -> Result<()> {
    setup().await?;

    let post_id = "cursorhalt";
    let uri = post_uri_builder(random_user_id(), post_id.to_string());

    let store = new_in_memory_store();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = create_mock_handler(
        Err(EventProcessorError::IndexOperationFailed(
            true,
            "simulated infra failure".to_string(),
        )),
        Some(post_id),
    );
    let processor =
        build_processor_at_cursor(10, store.clone(), handler.clone(), shutdown_rx).await?;

    let result = processor
        .process_event_lines(vec![format!("PUT {uri}"), "cursor: 11".to_string()])
        .await;
    assert!(
        result.is_err(),
        "Should-not-retry-now error must propagate and stop the batch"
    );

    assert_eq!(
        stored_cursor(&processor.homeserver.id).await?,
        10,
        "Cursor must not advance past events the stopped batch never processed"
    );

    let _ = shutdown_tx.send(true);
    Ok(())
}
