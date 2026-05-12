use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use nexus_common::db::{exec_single_row, graph::Query, queries, RedisOps};
use nexus_common::models::event::{Event, EventProcessorError};
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::user::{user_hs_cursor_key, UserDetails};
use nexus_common::types::DynError;
use nexus_watcher::events::retry::{InitialBackoff, RetryScheduler};
use nexus_watcher::events::EventHandler;
use nexus_watcher::service::indexer::{KeyBasedEventProcessor, RunError, TEventProcessor};
use pubky::{Event as StreamEvent, EventCursor, EventType, Keypair, PubkyResource, PublicKey};
use pubky_app_specs::PubkyId;
use tokio::sync::watch;

use crate::service::utils::{
    create_mock_handler, create_random_homeservers_and_persist, new_in_memory_store, setup,
    MockEventProcessorResult, MockKeyBasedEventSource,
};

/// Verifies `TEventProcessor::run` maps elapsed execution to a timeout error.
#[tokio_shared_rt::test(shared)]
async fn processor_run_returns_timeout_error() -> Result<(), DynError> {
    setup().await?;

    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let mut processors = Vec::new();
    create_random_homeservers_and_persist(
        &mut processors,
        Some(Duration::from_millis(50)),
        MockEventProcessorResult::Success,
        Some(Duration::from_millis(1)),
        shutdown_rx,
        None,
    )
    .await;

    let err = Arc::new(processors.pop().expect("processor should be created"))
        .run()
        .await
        .unwrap_err();

    assert!(err.is_timeout(), "expected timeout, got {err:?}");
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn key_based_processor_skips_unrecognized_events() -> Result<(), DynError> {
    setup().await?;

    // Create a homeserver with one hosted user to resolve during the run.
    let (_hs_keypair, homeserver) = create_homeserver().await?;
    let user_id = create_user_on_homeserver(&homeserver).await?;

    // Return one unrecognized event followed by one valid pubky.app event for the same user.
    let source = Arc::new(
        MockKeyBasedEventSource::default()
            .with_events(vec![vec![
                stream_event(1, &user_id, "/pub/other.app/profile.json")?,
                stream_event(2, &user_id, "/pub/pubky.app/profile.json")?,
            ]])
            .await,
    );

    let handler = create_mock_handler(Ok(()), None);
    let processor = processor(homeserver, handler.clone(), source.clone());

    processor.run().await?;

    // The unrecognized event is skipped, while the valid event is handled.
    assert_eq!(handler.get_handle_count(), 1);

    // The processor fetched events only for the hosted user.
    assert_eq!(source.calls().await, vec![user_id]);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn key_based_processor_stops_mismatched_user_stream_but_continues_other_users(
) -> Result<(), DynError> {
    setup().await?;

    // Create a homeserver with two hosted users to resolve during the run.
    let (_hs_keypair, homeserver) = create_homeserver().await?;
    let user_a_id = create_user_on_homeserver(&homeserver).await?;
    let user_b_id = create_user_on_homeserver(&homeserver).await?;

    // This ID is not hosted on the homeserver; it simulates a malicious or broken event source.
    let user_c_id = Keypair::random().public_key().to_z32();

    // For the first hosted user, return an event whose URI belongs to a different user.
    // The following valid event for the same hosted user must not be processed after that mismatch.
    let source = Arc::new(
        MockKeyBasedEventSource::default()
            .with_user_events(vec![
                (
                    user_a_id.clone(),
                    vec![
                        stream_event(1, &user_c_id, "/pub/pubky.app/profile.json")?,
                        stream_event(2, &user_a_id, "/pub/pubky.app/profile.json")?,
                    ],
                ),
                // For the second hosted user, return a valid event to prove processing continues.
                (
                    user_b_id.clone(),
                    vec![stream_event(3, &user_b_id, "/pub/pubky.app/profile.json")?],
                ),
            ])
            .await,
    );

    // Wire the processor to the user-keyed mock source and handler.
    let handler = create_mock_handler(Ok(()), None);
    let hs_id = homeserver.id.to_string();
    let processor = processor(homeserver, handler.clone(), source.clone());

    // Run one processing pass. User-level mismatches should be logged and skipped, not fail the run.
    let result = processor.run().await;

    assert!(result.is_ok());

    // Both hosted users were fetched from the same homeserver despite the first user's mismatch.
    let calls = source.calls().await;
    assert_eq!(calls.len(), 2);
    assert!(calls.contains(&user_a_id));
    assert!(calls.contains(&user_b_id));

    // Only the other user's event was handled; the valid event after the mismatch was skipped.
    let handled_uris = handler.get_handled_uris();
    assert_eq!(handled_uris.len(), 1);
    assert!(handled_uris.iter().all(|uri| !uri.contains(&user_a_id)));
    assert!(handled_uris.iter().any(|uri| uri.contains(&user_b_id)));

    // The mismatched user's cursor must not be persisted: the bad event is the first in the
    // batch, so `latest_cursor` is never set and no write to the USER_HS_CURSOR set should occur.
    let cursor_a =
        UserDetails::check_sorted_set_member(None, &user_hs_cursor_key(&user_a_id), &[&hs_id])
            .await?;
    assert!(
        cursor_a.is_none(),
        "user_a cursor must not be advanced past the mismatched event, got {cursor_a:?}",
    );

    Ok(())
}

/// Verifies an empty hosted-user set exits successfully without fetching events.
#[tokio_shared_rt::test(shared)]
async fn key_based_processor_returns_ok_without_users() -> Result<(), DynError> {
    setup().await?;

    let (_hs_keypair, homeserver) = create_homeserver().await?;
    let source = Arc::new(MockKeyBasedEventSource::default());
    let handler = create_mock_handler(Ok(()), None);
    let processor = processor(homeserver, handler.clone(), source.clone());

    processor.run().await?;

    assert!(source.calls().await.is_empty());
    assert_eq!(handler.get_handle_count(), 0);

    Ok(())
}

/// Verifies invalid resolved user IDs are skipped while valid users still run.
#[tokio_shared_rt::test(shared)]
async fn key_based_processor_skips_invalid_resolved_user_id() -> Result<(), DynError> {
    setup().await?;

    let (_hs_keypair, homeserver) = create_homeserver().await?;
    let valid_user_id = create_user_on_homeserver(&homeserver).await?;
    let invalid_user_id = "not-a-pubky-user";
    create_invalid_user_on_homeserver(&homeserver, invalid_user_id).await?;

    let source = Arc::new(
        MockKeyBasedEventSource::default()
            .with_user_events(vec![(
                valid_user_id.clone(),
                vec![stream_event(
                    1,
                    &valid_user_id,
                    "/pub/pubky.app/profile.json",
                )?],
            )])
            .await,
    );
    let handler = create_mock_handler(Ok(()), None);
    let processor = processor(homeserver, handler.clone(), source.clone());

    processor.run().await?;

    assert_eq!(source.calls().await, vec![valid_user_id]);
    assert_eq!(handler.get_handle_count(), 1);

    Ok(())
}

/// Verifies Redis cursor read failures abort before fetching user events.
#[tokio_shared_rt::test(shared)]
async fn key_based_processor_propagates_cursor_read_errors() -> Result<(), DynError> {
    setup().await?;

    let (_hs_keypair, homeserver) = create_homeserver().await?;
    let user_id = create_user_on_homeserver(&homeserver).await?;
    let cursor_key = user_hs_cursor_key(&user_id);
    test_user_details(&user_id)?
        .put_index_json(&cursor_key, Some("Sorted".into()), None)
        .await?;

    let source = Arc::new(MockKeyBasedEventSource::default());
    let handler = create_mock_handler(Ok(()), None);
    let processor = processor(homeserver, handler, source.clone());

    let err = processor.run().await.unwrap_err();

    assert_internal_index_operation_failed(err);
    assert!(source.calls().await.is_empty());

    Ok(())
}

/// Verifies stored per-user cursors and configured limits are passed to the source.
#[tokio_shared_rt::test(shared)]
async fn key_based_processor_passes_stored_cursor_and_limit_to_source() -> Result<(), DynError> {
    setup().await?;

    let (_hs_keypair, homeserver) = create_homeserver().await?;
    let hs_id = homeserver.id.to_string();
    let user_id = create_user_on_homeserver(&homeserver).await?;
    let cursor_key = user_hs_cursor_key(&user_id);
    UserDetails::put_index_sorted_set(&cursor_key, &[(42.0, hs_id.as_str())], None, None).await?;

    let source = Arc::new(MockKeyBasedEventSource::default());
    let handler = create_mock_handler(Ok(()), None);
    let processor = processor_with_limit(homeserver, handler, source.clone(), 17);

    processor.run().await?;

    assert_eq!(source.call_details().await, vec![(user_id, 42, 17)]);

    Ok(())
}

/// Verifies successful event processing persists the last stream cursor.
#[tokio_shared_rt::test(shared)]
async fn key_based_processor_persists_latest_cursor_after_success() -> Result<(), DynError> {
    setup().await?;

    let (_hs_keypair, homeserver) = create_homeserver().await?;
    let hs_id = homeserver.id.to_string();
    let user_id = create_user_on_homeserver(&homeserver).await?;
    let source = Arc::new(
        MockKeyBasedEventSource::default()
            .with_events(vec![vec![
                stream_event(1, &user_id, "/pub/pubky.app/profile.json")?,
                stream_event(4, &user_id, "/pub/pubky.app/profile.json")?,
            ]])
            .await,
    );
    let handler = create_mock_handler(Ok(()), None);
    let processor = processor(homeserver, handler.clone(), source);

    processor.run().await?;

    assert_eq!(handler.get_handle_count(), 2);
    assert_eq!(user_cursor(&user_id, &hs_id).await?, Some(4));

    Ok(())
}

/// Verifies cursor persistence stops at the last safe event before a mismatch.
#[tokio_shared_rt::test(shared)]
async fn key_based_processor_persists_last_safe_cursor_before_mismatch() -> Result<(), DynError> {
    setup().await?;

    let (_hs_keypair, homeserver) = create_homeserver().await?;
    let hs_id = homeserver.id.to_string();
    let user_id = create_user_on_homeserver(&homeserver).await?;
    let mismatched_user_id = Keypair::random().public_key().to_z32();
    let source = Arc::new(
        MockKeyBasedEventSource::default()
            .with_events(vec![vec![
                stream_event(5, &user_id, "/pub/pubky.app/profile.json")?,
                stream_event(6, &mismatched_user_id, "/pub/pubky.app/profile.json")?,
            ]])
            .await,
    );
    let handler = create_mock_handler(Ok(()), None);
    let processor = processor(homeserver, handler.clone(), source);

    processor.run().await?;

    assert_eq!(handler.get_handle_count(), 1);
    assert_eq!(user_cursor(&user_id, &hs_id).await?, Some(5));

    Ok(())
}

/// Verifies infrastructure fetch errors abort the homeserver run immediately.
#[tokio_shared_rt::test(shared)]
async fn key_based_processor_aborts_on_infrastructure_fetch_error() -> Result<(), DynError> {
    setup().await?;

    let (_hs_keypair, homeserver) = create_homeserver().await?;
    create_user_on_homeserver(&homeserver).await?;
    create_user_on_homeserver(&homeserver).await?;
    let source = Arc::new(
        MockKeyBasedEventSource::default()
            .with_results(vec![Err(EventProcessorError::IndexOperationFailed(
                true,
                "redis unavailable".into(),
            ))])
            .await,
    );
    let handler = create_mock_handler(Ok(()), None);
    let processor = processor(homeserver, handler.clone(), source.clone());

    let err = processor.run().await.unwrap_err();

    assert_internal_infrastructure_index_operation_failed(err);
    assert_eq!(source.calls().await.len(), 1);
    assert_eq!(handler.get_handle_count(), 0);

    Ok(())
}

/// Verifies non-infrastructure fetch errors skip only the affected user.
#[tokio_shared_rt::test(shared)]
async fn key_based_processor_continues_after_non_infrastructure_fetch_error() -> Result<(), DynError>
{
    setup().await?;

    let (_hs_keypair, homeserver) = create_homeserver().await?;
    let user_a_id = create_user_on_homeserver(&homeserver).await?;
    let user_b_id = create_user_on_homeserver(&homeserver).await?;
    let source = Arc::new(
        MockKeyBasedEventSource::default()
            .with_user_results(vec![
                (
                    user_a_id.clone(),
                    Err(EventProcessorError::Generic("bad user stream".into())),
                ),
                (
                    user_b_id.clone(),
                    Ok(vec![stream_event(
                        9,
                        &user_b_id,
                        "/pub/pubky.app/profile.json",
                    )?]),
                ),
            ])
            .await,
    );
    let handler = create_mock_handler(Ok(()), None);
    let processor = processor(homeserver, handler.clone(), source.clone());

    processor.run().await?;

    let calls = source.calls().await;
    assert_eq!(calls.len(), 2);
    assert!(calls.contains(&user_a_id));
    assert!(calls.contains(&user_b_id));
    assert_eq!(handler.get_handle_count(), 1);

    Ok(())
}

/// Verifies infrastructure handler failures abort without advancing the cursor.
#[tokio_shared_rt::test(shared)]
async fn key_based_processor_aborts_and_keeps_cursor_on_infrastructure_handler_error(
) -> Result<(), DynError> {
    setup().await?;

    let (_hs_keypair, homeserver) = create_homeserver().await?;
    let hs_id = homeserver.id.to_string();
    let user_id = create_user_on_homeserver(&homeserver).await?;
    let source = Arc::new(
        MockKeyBasedEventSource::default()
            .with_events(vec![vec![stream_event(
                9,
                &user_id,
                "/pub/pubky.app/profile.json",
            )?]])
            .await,
    );
    let handler = create_mock_handler(
        Err(EventProcessorError::IndexOperationFailed(
            true,
            "redis unavailable".into(),
        )),
        None,
    );
    let processor = processor(homeserver, handler.clone(), source);

    let err = processor.run().await.unwrap_err();

    assert_internal_infrastructure_index_operation_failed(err);
    assert_eq!(handler.get_handle_count(), 1);
    assert_eq!(user_cursor(&user_id, &hs_id).await?, None);

    Ok(())
}

/// Verifies an already-signaled shutdown exits before fetching any user events.
#[tokio_shared_rt::test(shared)]
async fn key_based_processor_does_not_fetch_when_shutdown_is_already_set() -> Result<(), DynError> {
    setup().await?;

    let (_hs_keypair, homeserver) = create_homeserver().await?;
    create_user_on_homeserver(&homeserver).await?;
    let source = Arc::new(MockKeyBasedEventSource::default());
    let handler = create_mock_handler(Ok(()), None);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    shutdown_tx
        .send(true)
        .expect("shutdown receiver should exist");
    let processor =
        processor_with_shutdown(homeserver, handler.clone(), source.clone(), shutdown_rx);

    processor.run().await?;

    assert!(source.calls().await.is_empty());
    assert_eq!(handler.get_handle_count(), 0);

    Ok(())
}

/// Verifies shutdown during one user stops that stream and prevents later users.
#[tokio_shared_rt::test(shared)]
async fn key_based_processor_stops_current_and_next_users_after_shutdown() -> Result<(), DynError> {
    setup().await?;

    let (_hs_keypair, homeserver) = create_homeserver().await?;
    let hs_id = homeserver.id.to_string();
    let user_a_id = create_user_on_homeserver(&homeserver).await?;
    let user_b_id = create_user_on_homeserver(&homeserver).await?;
    let source = Arc::new(
        MockKeyBasedEventSource::default()
            .with_user_events(vec![
                (
                    user_a_id.clone(),
                    vec![
                        stream_event(1, &user_a_id, "/pub/pubky.app/profile.json")?,
                        stream_event(2, &user_a_id, "/pub/pubky.app/profile.json")?,
                    ],
                ),
                (
                    user_b_id.clone(),
                    vec![
                        stream_event(1, &user_b_id, "/pub/pubky.app/profile.json")?,
                        stream_event(2, &user_b_id, "/pub/pubky.app/profile.json")?,
                    ],
                ),
            ])
            .await,
    );
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = Arc::new(ShutdownOnFirstHandle::new(shutdown_tx));
    let processor =
        processor_with_shutdown(homeserver, handler.clone(), source.clone(), shutdown_rx);

    processor.run().await?;

    let calls = source.calls().await;
    assert_eq!(calls.len(), 1);
    assert_eq!(handler.handle_count(), 1);
    assert_eq!(user_cursor(&calls[0], &hs_id).await?, Some(1));

    Ok(())
}

async fn create_homeserver() -> Result<(Keypair, Homeserver), DynError> {
    let keypair = Keypair::random();
    let homeserver_id = PubkyId::try_from(keypair.public_key().to_z32().as_str())?;
    let homeserver = Homeserver::new(homeserver_id);
    homeserver.put_to_graph().await?;
    Ok((keypair, homeserver))
}

async fn create_user_on_homeserver(homeserver: &Homeserver) -> Result<String, DynError> {
    let user_id = PubkyId::try_from(Keypair::random().public_key().to_z32().as_str())?;
    let user = UserDetails {
        id: user_id.clone(),
        name: "key-based-processor-test-user".into(),
        bio: None,
        status: None,
        links: None,
        image: None,
        indexed_at: Utc::now().timestamp_millis(),
    };

    exec_single_row(queries::put::create_user(&user)?).await?;
    exec_single_row(queries::put::set_user_homeserver(&user_id, &homeserver.id)).await?;

    Ok(user_id.to_string())
}

async fn create_invalid_user_on_homeserver(
    homeserver: &Homeserver,
    user_id: &str,
) -> Result<(), DynError> {
    exec_single_row(
        Query::new(
            "create_invalid_key_based_user",
            "MERGE (u:User {id: $id}) SET u.name = $name",
        )
        .param("id", user_id.to_string())
        .param("name", "invalid-key-based-processor-test-user".to_string()),
    )
    .await?;
    exec_single_row(queries::put::set_user_homeserver(user_id, &homeserver.id)).await?;

    Ok(())
}

fn test_user_details(user_id: &str) -> Result<UserDetails, DynError> {
    Ok(UserDetails {
        id: PubkyId::try_from(user_id)?,
        name: "key-based-processor-test-user".into(),
        bio: None,
        status: None,
        links: None,
        image: None,
        indexed_at: Utc::now().timestamp_millis(),
    })
}

async fn user_cursor(user_id: &str, hs_id: &str) -> Result<Option<isize>, DynError> {
    Ok(UserDetails::check_sorted_set_member(None, &user_hs_cursor_key(user_id), &[hs_id]).await?)
}

fn stream_event(cursor: u64, user_id: &str, path: &str) -> Result<StreamEvent, DynError> {
    let user_pk: PublicKey = user_id.parse()?;

    Ok(StreamEvent {
        event_type: EventType::Delete,
        resource: PubkyResource::new(user_pk, path)?,
        cursor: EventCursor::new(cursor),
    })
}

fn processor(
    homeserver: Homeserver,
    handler: Arc<dyn EventHandler>,
    source: Arc<MockKeyBasedEventSource>,
) -> Arc<KeyBasedEventProcessor> {
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    processor_with_options(homeserver, handler, source, 100, shutdown_rx)
}

fn processor_with_limit(
    homeserver: Homeserver,
    handler: Arc<dyn EventHandler>,
    source: Arc<MockKeyBasedEventSource>,
    limit: u16,
) -> Arc<KeyBasedEventProcessor> {
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    processor_with_options(homeserver, handler, source, limit, shutdown_rx)
}

fn processor_with_shutdown(
    homeserver: Homeserver,
    handler: Arc<dyn EventHandler>,
    source: Arc<MockKeyBasedEventSource>,
    shutdown_rx: watch::Receiver<bool>,
) -> Arc<KeyBasedEventProcessor> {
    processor_with_options(homeserver, handler, source, 100, shutdown_rx)
}

fn processor_with_options(
    homeserver: Homeserver,
    handler: Arc<dyn EventHandler>,
    source: Arc<MockKeyBasedEventSource>,
    limit: u16,
    shutdown_rx: watch::Receiver<bool>,
) -> Arc<KeyBasedEventProcessor> {
    Arc::new(KeyBasedEventProcessor {
        homeserver,
        limit,
        files_path: PathBuf::from("/tmp/nexus-watcher-test"),
        event_handler: handler,
        event_source: source,
        retry_scheduler: Arc::new(RetryScheduler::new(
            new_in_memory_store(),
            InitialBackoff {
                missing_dep_ms: 60_000,
                transient_ms: 10_000,
            },
        )),
        shutdown_rx,
    })
}

fn assert_internal_index_operation_failed(err: RunError) {
    match err {
        RunError::Internal(EventProcessorError::IndexOperationFailed(_, _)) => {}
        other => panic!("expected internal index operation failure, got {other:?}"),
    }
}

fn assert_internal_infrastructure_index_operation_failed(err: RunError) {
    match err {
        RunError::Internal(EventProcessorError::IndexOperationFailed(true, _)) => {}
        other => panic!("expected internal infrastructure index operation failure, got {other:?}"),
    }
}

/// Test handler that signals shutdown after handling its first event.
///
/// This lets shutdown-path tests verify that the processor persists the first
/// safe cursor, stops the current user stream, and does not fetch later users.
struct ShutdownOnFirstHandle {
    shutdown_tx: watch::Sender<bool>,
    handle_count: AtomicUsize,
}

impl ShutdownOnFirstHandle {
    fn new(shutdown_tx: watch::Sender<bool>) -> Self {
        Self {
            shutdown_tx,
            handle_count: AtomicUsize::new(0),
        }
    }

    fn handle_count(&self) -> usize {
        self.handle_count.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl EventHandler for ShutdownOnFirstHandle {
    async fn handle(&self, _event: &Event) -> Result<(), EventProcessorError> {
        if self.handle_count.fetch_add(1, Ordering::SeqCst) == 0 {
            let _ = self.shutdown_tx.send(true);
        }

        Ok(())
    }
}
