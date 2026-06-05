use crate::event_processor::utils::{default_ingestor_tests, default_moderation_tests};
use crate::service::utils::HS_IDS;
use crate::service::utils::{create_mock_event_processors, setup, MockEventProcessorRunner};
use anyhow::Result;
use chrono::Utc;
use nexus_common::db::{exec_single_row, queries};
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::user::UserDetails;
use nexus_common::types::DynError;
use nexus_watcher::events::retry::{InitialBackoff, RedisRetryStore, RetryScheduler, RetryStore};
use nexus_watcher::events::{DefaultEventHandler, EventHandler};
use nexus_watcher::service::indexer::PubkyKeyBasedEventSource;
use nexus_watcher::service::runner::HomeserverBackoff;
use nexus_watcher::service::runner::UserNotFoundBackoff;
use nexus_watcher::service::{KeyBasedEventProcessorRunner, TEventProcessorRunner};
use pubky::Keypair;
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio_shared_rt::test(shared)]
async fn test_event_processor_runner_default_homeserver_excluded() -> Result<(), DynError> {
    // Initialize the test
    setup().await?;

    let event_handler: Arc<dyn EventHandler> = Arc::new(DefaultEventHandler::new(
        default_moderation_tests(),
        default_ingestor_tests(),
    ));
    let store: Arc<dyn RetryStore> = Arc::new(RedisRetryStore::new());
    let retry_scheduler = Arc::new(RetryScheduler::new(
        store,
        InitialBackoff {
            missing_dep_ms: 60_000,
            transient_ms: 10_000,
        },
    ));
    let runner = KeyBasedEventProcessorRunner {
        limit: 1000,
        monitored_hs_limit: HS_IDS.len(),
        files_path: PathBuf::from("/tmp/nexus-watcher-test"),
        event_handler,
        event_source: Arc::new(PubkyKeyBasedEventSource),
        shutdown_rx: tokio::sync::watch::channel(false).1,
        default_homeserver: PubkyId::try_from(HS_IDS[3]).unwrap(),
        external_hs_pk_blacklist: Arc::new(Vec::new()),
        backoff: Mutex::new(HomeserverBackoff::default()),
        user_not_found_backoff: Arc::new(UserNotFoundBackoff::default()),
        retry_scheduler,
    };

    // Persist the homeservers
    for hs_id in HS_IDS {
        let hs = Homeserver::new(PubkyId::try_from(hs_id).unwrap());
        hs.put_to_graph().await.unwrap();
    }

    // The default homeserver should be excluded from the list
    let hs_ids = runner.pre_run().await?;
    assert!(
        !hs_ids.contains(&HS_IDS[3].to_string()),
        "Default homeserver should be excluded from pre_run"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_event_processor_runner_blacklisted_homeserver_excluded() -> Result<(), DynError> {
    // Initialize the test
    setup().await?;

    let event_handler: Arc<dyn EventHandler> = Arc::new(DefaultEventHandler::new(
        default_moderation_tests(),
        default_ingestor_tests(),
    ));
    let store: Arc<dyn RetryStore> = Arc::new(RedisRetryStore::new());
    let retry_scheduler = Arc::new(RetryScheduler::new(
        store,
        InitialBackoff {
            missing_dep_ms: 60_000,
            transient_ms: 10_000,
        },
    ));

    // Fresh random HSs so this test's active-user graph state is isolated.
    let blacklisted_hs = random_homeserver_id();
    let allowed_hs = random_homeserver_id();
    let runner = KeyBasedEventProcessorRunner {
        limit: 1000,
        monitored_hs_limit: 100,
        files_path: PathBuf::from("/tmp/nexus-watcher-test"),
        event_handler,
        event_source: Arc::new(PubkyKeyBasedEventSource),
        shutdown_rx: tokio::sync::watch::channel(false).1,
        default_homeserver: PubkyId::try_from(HS_IDS[3]).unwrap(),
        external_hs_pk_blacklist: Arc::new(vec![blacklisted_hs.clone()]),
        backoff: Mutex::new(HomeserverBackoff::default()),
        user_not_found_backoff: Arc::new(UserNotFoundBackoff::default()),
        retry_scheduler,
    };

    // Both HSs need a hosted user to count as "active" in `get_all_active_from_graph`.
    Homeserver::new(blacklisted_hs.clone())
        .put_to_graph()
        .await?;
    Homeserver::new(allowed_hs.clone()).put_to_graph().await?;
    create_active_user_on_homeserver(&blacklisted_hs).await?;
    create_active_user_on_homeserver(&allowed_hs).await?;

    let hs_ids = runner.pre_run().await?;
    assert!(
        !hs_ids.contains(&blacklisted_hs.to_string()),
        "Blacklisted HS should be excluded from pre_run"
    );
    // The non-blacklisted active HS must still be present, proving the blacklist
    // (not just inactivity) removed the other one.
    assert!(
        hs_ids.contains(&allowed_hs.to_string()),
        "Non-blacklisted active HS should be included in pre_run"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_mock_event_processor_runner_default_homeserver_excluded() -> Result<(), DynError> {
    // Initialize the test
    setup().await?;

    let event_processors = create_mock_event_processors(None, tokio::sync::watch::channel(false).1)
        .into_iter()
        .map(Arc::new)
        .collect();

    let runner = MockEventProcessorRunner {
        event_processors,
        monitored_hs_limit: 100,
        shutdown_rx: tokio::sync::watch::channel(false).1,
    };

    // Persist the homeservers
    for hs_id in HS_IDS {
        let hs = Homeserver::new(PubkyId::try_from(hs_id).unwrap());
        hs.put_to_graph().await.unwrap();
    }

    // The default homeserver (HS_IDS[0]) should be excluded from the list
    let hs_ids = runner.hs_by_priority().await?;
    assert!(
        !hs_ids.contains(&HS_IDS[0].to_string()),
        "Default homeserver should be excluded from hs_by_priority"
    );

    Ok(())
}

fn random_homeserver_id() -> PubkyId {
    PubkyId::try_from(Keypair::random().public_key().to_z32().as_str()).unwrap()
}

/// Creates a user node with a `HOSTED_BY` edge to `hs_id`, making the HS
/// "active" for `get_all_active_from_graph`.
async fn create_active_user_on_homeserver(hs_id: &PubkyId) -> Result<(), DynError> {
    let user_id = random_homeserver_id();
    let user = UserDetails {
        id: user_id.clone(),
        name: "prioritization-test-user".into(),
        bio: None,
        status: None,
        links: None,
        image: None,
        indexed_at: Utc::now().timestamp_millis(),
    };

    exec_single_row(queries::put::create_user(&user)?).await?;
    exec_single_row(queries::put::set_user_homeserver(&user_id, hs_id)).await?;

    Ok(())
}
