use crate::event_processor::utils::default_moderation_tests;
use crate::service::utils::HS_IDS;
use crate::service::utils::{create_mock_event_processors, setup, MockEventProcessorRunner};

use anyhow::Result;
use chrono::Utc;
use nexus_common::db::graph::exec::exec_single_row;
use nexus_common::db::graph::Query;
use nexus_common::models::homeserver::{Homeserver, HsBlacklist};
use nexus_common::models::traits::Collection;
use nexus_common::models::user::{set_user_homeserver, UserDetails};
use nexus_common::types::DynError;
use nexus_common::utils::test_utils::{default_ingestor_tests, random_pubky_id};
use nexus_common::DEFAULT_MAX_FILE_SIZE;
use nexus_watcher::events::retry::{InitialBackoff, RedisRetryStore, RetryScheduler, RetryStore};
use nexus_watcher::events::{DefaultEventHandler, EventHandler};
use nexus_watcher::service::indexer::PubkyKeyBasedEventSource;
use nexus_watcher::service::runner::HomeserverBackoff;
use nexus_watcher::service::runner::UserNotFoundBackoff;
use nexus_watcher::service::{KeyBasedEventProcessorRunner, TEventProcessorRunner};
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio_shared_rt::test(shared)]
async fn test_event_processor_runner_primary_homeserver_excluded() -> Result<(), DynError> {
    // Initialize the test
    setup().await?;

    let event_handler: Arc<dyn EventHandler> = Arc::new(DefaultEventHandler::new(
        default_moderation_tests(),
        default_ingestor_tests(),
        DEFAULT_MAX_FILE_SIZE,
        PathBuf::from("/tmp/nexus-watcher-test"),
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
        event_handler,
        event_source: Arc::new(PubkyKeyBasedEventSource),
        shutdown_rx: tokio::sync::watch::channel(false).1,
        primary_homeserver: PubkyId::try_from(HS_IDS[3]).unwrap(),
        hs_blacklist: HsBlacklist::default(),
        backoff: Mutex::new(HomeserverBackoff::default()),
        user_not_found_backoff: Arc::new(UserNotFoundBackoff::default()),
        retry_scheduler,
    };

    // Persist the homeservers
    for hs_id in HS_IDS {
        let hs = Homeserver::new(PubkyId::try_from(hs_id).unwrap());
        hs.put_to_graph().await.unwrap();
    }

    // The primary homeserver should be excluded from the list
    let hs_ids = runner.pre_run().await?;
    assert!(
        !hs_ids.contains(&HS_IDS[3].to_string()),
        "Primary homeserver should be excluded from pre_run"
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
        DEFAULT_MAX_FILE_SIZE,
        PathBuf::from("/tmp/nexus-watcher-test"),
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
    let blacklisted_hs = random_pubky_id();
    let allowed_hs = random_pubky_id();
    let runner = KeyBasedEventProcessorRunner {
        limit: 1000,
        monitored_hs_limit: 100,
        event_handler,
        event_source: Arc::new(PubkyKeyBasedEventSource),
        shutdown_rx: tokio::sync::watch::channel(false).1,
        primary_homeserver: PubkyId::try_from(HS_IDS[3]).unwrap(),
        hs_blacklist: HsBlacklist::new([blacklisted_hs.clone()]),
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
async fn test_mock_event_processor_runner_primary_homeserver_excluded() -> Result<(), DynError> {
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

    // The primary homeserver (HS_IDS[0]) should be excluded from the list
    let hs_ids = runner.hs_by_priority().await?;
    assert!(
        !hs_ids.contains(&HS_IDS[0].to_string()),
        "Primary homeserver should be excluded from hs_by_priority"
    );

    Ok(())
}

/// Ordering is by aggregate hosted trust, so a homeserver hosting a couple of
/// well-connected users must outrank one hosting a crowd of unranked keys —
/// otherwise minting keys buys polling priority, which is the whole point of
/// ranking here.
///
/// The shared fixture cannot cover this: no fixture user has a `HOSTED_BY` edge,
/// so every homeserver in it aggregates to 0.0 trust. This test builds its own
/// graph on two fresh random homeservers instead.
#[tokio_shared_rt::test(shared)]
async fn test_event_processor_runner_orders_homeservers_by_hosted_trust() -> Result<(), DynError> {
    setup().await?;

    let trusted_hs = random_pubky_id();
    let crowded_hs = random_pubky_id();
    Homeserver::new(trusted_hs.clone()).put_to_graph().await?;
    Homeserver::new(crowded_hs.clone()).put_to_graph().await?;

    // Two ranked users against eight unranked ones: the crowded HS wins on raw
    // count, so if it still sorts first the trust term is not being applied.
    // Values deliberately far below the fixture's top score: these users are
    // deleted below, but while they exist they must not reorder the shared
    // ranking that other suites assert on.
    let mut created = vec![
        create_active_user_on_homeserver_with_trust(&trusted_hs, Some(0.05)).await?,
        create_active_user_on_homeserver_with_trust(&trusted_hs, Some(0.02)).await?,
    ];
    for _ in 0..8 {
        created.push(create_active_user_on_homeserver_with_trust(&crowded_hs, None).await?);
    }

    // Read before cleanup, assert after, so a failed assertion still cleans up.
    let hs_ids = Homeserver::get_all_active_from_graph().await?;
    delete_users(&created).await?;
    let rank = |id: &PubkyId| {
        hs_ids
            .iter()
            .position(|hs| hs == id.as_ref())
            .unwrap_or_else(|| panic!("{id} should be active, got {hs_ids:?}"))
    };

    assert!(
        rank(&trusted_hs) < rank(&crowded_hs),
        "the HS hosting trust should be polled before the one hosting more keys, got {hs_ids:?}"
    );

    Ok(())
}

/// Creates a user node with a `HOSTED_BY` edge to `hs_id`, making the HS
/// "active" for `get_all_active_from_graph`.
async fn create_active_user_on_homeserver(hs_id: &PubkyId) -> Result<(), DynError> {
    create_active_user_on_homeserver_with_trust(hs_id, None).await?;
    Ok(())
}

/// As [`create_active_user_on_homeserver`], but optionally scores the user.
///
/// `None` leaves `trust` unset rather than writing 0.0, which is what an
/// unranked account actually looks like: absent from the ranking, not scored
/// zero. The query under test coalesces the two, and this keeps the test honest
/// about which case it is exercising.
async fn create_active_user_on_homeserver_with_trust(
    hs_id: &PubkyId,
    trust: Option<f64>,
) -> Result<PubkyId, DynError> {
    let user_id = random_pubky_id();
    let user = UserDetails {
        id: user_id.clone(),
        name: "prioritization-test-user".into(),
        bio: None,
        status: None,
        links: None,
        image: None,
        indexed_at: Utc::now().timestamp_millis(),
    };

    user.put_to_graph().await?;
    set_user_homeserver(&user_id, hs_id).await?;

    if let Some(trust) = trust {
        let query = Query::new(
            "prioritization_test_set_trust",
            "MATCH (u:User {id: $id}) SET u.trust = $trust",
        )
        .param("id", user_id.to_string())
        .param("trust", trust);
        exec_single_row(query).await?;
    }

    Ok(user_id)
}

/// Removes users this file created, so a scored test user cannot leak into the
/// global trust ranking. `Sorted:Users:SocialGraph` is one shared key built from
/// `MATCH (u:User) WHERE u.trust > 0`, and `nexus-webapi`'s
/// `test_social_graph_status` asserts on positions in it — a stray 0.4 here ties
/// the fixture's top user and wins the `id ASC` tiebreak half the time.
async fn delete_users(user_ids: &[PubkyId]) -> Result<(), DynError> {
    let ids: Vec<String> = user_ids.iter().map(ToString::to_string).collect();
    let query = Query::new(
        "prioritization_test_delete_users",
        "MATCH (u:User) WHERE u.id IN $ids DETACH DELETE u",
    )
    .param("ids", ids);
    exec_single_row(query).await?;
    Ok(())
}
