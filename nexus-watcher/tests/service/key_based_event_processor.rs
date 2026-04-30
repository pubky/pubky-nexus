use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use nexus_common::db::{exec_single_row, queries, RedisOps};
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::user::{user_hs_cursor_key, UserDetails};
use nexus_common::types::DynError;
use nexus_watcher::events::retry::{InitialBackoff, RetryScheduler};
use nexus_watcher::events::EventHandler;
use nexus_watcher::service::indexer::{KeyBasedEventProcessor, TEventProcessor};
use pubky::{Event as StreamEvent, EventCursor, EventType, Keypair, PubkyResource, PublicKey};
use pubky_app_specs::PubkyId;

use crate::service::utils::{
    create_mock_handler, new_in_memory_store, setup, MockKeyBasedEventSource,
};

#[tokio_shared_rt::test(shared)]
async fn key_based_processor_skips_unrecognized_events() -> Result<(), DynError> {
    setup().await?;

    // Create a homeserver with one hosted user to resolve during the run.
    let (_hs_keypair, homeserver) = create_homeserver().await?;
    let user_id = create_user_on_homeserver(&homeserver).await?;

    // Return one unrecognized event followed by one valid pubky.app event for the same user.
    let source = Arc::new(MockKeyBasedEventSource::default().with_events(vec![vec![
        stream_event(1, &user_id, "/pub/other.app/profile.json")?,
        stream_event(2, &user_id, "/pub/pubky.app/profile.json")?,
    ]]));

    let handler = create_mock_handler(Ok(()), None);
    let processor = processor(homeserver, handler.clone(), source.clone());

    processor.run().await?;

    // The unrecognized event is skipped, while the valid event is handled.
    assert_eq!(handler.get_handle_count(), 1);

    // The processor fetched events only for the hosted user.
    assert_eq!(source.calls(), vec![user_id]);

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
    let source = Arc::new(MockKeyBasedEventSource::default().with_user_events(vec![
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
    ]));

    // Wire the processor to the user-keyed mock source and handler.
    let handler = create_mock_handler(Ok(()), None);
    let hs_id = homeserver.id.to_string();
    let processor = processor(homeserver, handler.clone(), source.clone());

    // Run one processing pass. User-level mismatches should be logged and skipped, not fail the run.
    let result = processor.run().await;

    assert!(result.is_ok());

    // Both hosted users were fetched from the same homeserver despite the first user's mismatch.
    let calls = source.calls();
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
    Arc::new(KeyBasedEventProcessor {
        homeserver,
        limit: 100,
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
        shutdown_rx: tokio::sync::watch::channel(false).1,
    })
}
