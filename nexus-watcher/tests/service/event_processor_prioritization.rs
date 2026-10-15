use crate::event_processor::utils::default_moderation_tests;
use crate::service::utils::HS_IDS;
use crate::service::utils::{create_mock_event_processors, setup, MockEventProcessorRunner};

use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use deadpool_redis::redis::AsyncCommands;
use nexus_common::db::get_redis_conn;
use nexus_common::db::graph::exec::exec_single_row;
use nexus_common::db::graph::Query;
use nexus_common::models::homeserver::{Homeserver, HsBlacklist};
use nexus_common::models::post::PostDetails;
use nexus_common::models::scope::{EverythingScope, HomeserverListScope, Scope, SeedHopsScope};
use nexus_common::models::traits::Collection;
use nexus_common::models::user::catch_up::{catching_up, caught_up, start_catch_up};
use nexus_common::models::user::stub::mark_stub;
use nexus_common::models::user::{set_user_homeserver, Epoch, UserDetails, UserIngestor};
use nexus_common::types::DynError;
use nexus_common::utils::test_utils::{default_ingestor_tests, random_pubky_id};
use nexus_common::DEFAULT_MAX_FILE_SIZE;
use nexus_watcher::events::retry::{InitialBackoff, RedisRetryStore, RetryScheduler, RetryStore};
use nexus_watcher::events::{DefaultEventHandler, EventHandler};
use nexus_watcher::service::indexer::PubkyKeyBasedEventSource;
use nexus_watcher::service::runner::HomeserverBackoff;
use nexus_watcher::service::runner::{UserNotFoundBackoff, UserResumePoints};
use nexus_watcher::service::{
    HsEventProcessorRunner, KeyBasedEventProcessorRunner, TEventProcessorRunner,
    KEY_BASED_RUN_BUDGET,
};
use pubky::Keypair;
use pubky_app_specs::PubkyId;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind, PubkyAppUser};
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
        scope: Arc::new(EverythingScope),
        hs_blacklist: HsBlacklist::default(),
        backoff: Mutex::new(HomeserverBackoff::default()),
        user_not_found_backoff: Arc::new(UserNotFoundBackoff::default()),
        user_resume_points: Arc::new(UserResumePoints::default()),
        budget: KEY_BASED_RUN_BUDGET,
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
        scope: Arc::new(EverythingScope),
        hs_blacklist: HsBlacklist::new([blacklisted_hs.clone()]),
        backoff: Mutex::new(HomeserverBackoff::default()),
        user_not_found_backoff: Arc::new(UserNotFoundBackoff::default()),
        user_resume_points: Arc::new(UserResumePoints::default()),
        budget: KEY_BASED_RUN_BUDGET,
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

    // Clean up before the `?` and before asserting: a failed read would otherwise
    // skip cleanup and leak scored users into the shared graph, which is the one
    // thing this test must not do.
    let hs_ids = Homeserver::get_all_active_from_graph().await;
    delete_users(&created).await?;
    let hs_ids = hs_ids?;
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
        deleted: false,
        epoch: Epoch::V0,
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
/// `test_social_graph_status` asserts on positions in it.
///
/// At the scores this file writes (0.05 and 0.02, both below the fixture's lowest
/// score of 0.1) a leak would not break that test today: the leaked users would
/// rank below every user it asserts on, and the
/// fixture's top user keeps it. The cleanup is here because that safety is a
/// coincidence of the current values, not a property — a future score at or above
/// the fixture's top of 0.4 would tie it and win the `id ASC` tiebreak, and one at
/// or above 0.1 would reorder the ranks the test asserts on. Keeping the graph
/// clean is cheaper than re-deriving that argument every time a value changes.
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

/// A key-based runner for `primary`, reading feeds as `scope` says.
fn key_based_runner(primary: &PubkyId, scope: Arc<dyn Scope>) -> KeyBasedEventProcessorRunner {
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
    KeyBasedEventProcessorRunner {
        limit: 1000,
        monitored_hs_limit: 100,
        event_handler,
        event_source: Arc::new(PubkyKeyBasedEventSource),
        shutdown_rx: tokio::sync::watch::channel(false).1,
        primary_homeserver: primary.clone(),
        scope,
        hs_blacklist: HsBlacklist::default(),
        backoff: Mutex::new(HomeserverBackoff::default()),
        user_not_found_backoff: Arc::new(UserNotFoundBackoff::default()),
        user_resume_points: Arc::new(UserResumePoints::default()),
        budget: KEY_BASED_RUN_BUDGET,
        retry_scheduler,
    }
}

/// Persists `count` homeservers, each with an active user, so the key-based runner sees them.
async fn active_homeservers(count: usize) -> Result<Vec<PubkyId>, DynError> {
    let mut homeservers = Vec::with_capacity(count);
    for _ in 0..count {
        let hs = random_pubky_id();
        Homeserver::new(hs.clone()).put_to_graph().await?;
        create_active_user_on_homeserver(&hs).await?;
        homeservers.push(hs);
    }
    Ok(homeservers)
}

/// A homeserver scope reads its listed homeservers as whole feeds, so the key-based
/// runner leaves them out; the primary homeserver is polled per user like any other.
#[tokio_shared_rt::test(shared)]
async fn test_key_based_runner_skips_the_homeservers_a_scope_reads_whole() -> Result<(), DynError> {
    setup().await?;
    let [primary, listed, other] = active_homeservers(3).await?.try_into().unwrap();

    let scope = Arc::new(HomeserverListScope::new(std::slice::from_ref(&listed)));
    let hs_ids = key_based_runner(&primary, scope).pre_run().await?;

    assert!(
        !hs_ids.contains(&listed.to_string()),
        "a listed HS is read whole"
    );
    assert!(
        hs_ids.contains(&primary.to_string()),
        "the primary HS is not read whole"
    );
    assert!(hs_ids.contains(&other.to_string()));
    Ok(())
}

/// A seed scope reads no feed whole: members on the primary homeserver are polled
/// through their own streams too.
#[tokio_shared_rt::test(shared)]
async fn test_key_based_runner_polls_the_primary_homeserver_for_a_seed_scope(
) -> Result<(), DynError> {
    setup().await?;
    let [primary] = active_homeservers(1).await?.try_into().unwrap();

    let everything = key_based_runner(&primary, Arc::new(EverythingScope));
    assert!(!everything.pre_run().await?.contains(&primary.to_string()));

    let seed = Arc::new(SeedHopsScope::new(random_pubky_id(), 2));
    let hs_ids = key_based_runner(&primary, seed).pre_run().await?;
    assert!(hs_ids.contains(&primary.to_string()));
    Ok(())
}

/// The key-based runner never reads a homeserver read whole, also while users there are
/// catching up: the whole-feed processor reads their streams, so no record is handled by
/// two tasks at once.
#[tokio_shared_rt::test(shared)]
async fn test_key_based_runner_leaves_users_catching_up_to_the_whole_feed_reader(
) -> Result<(), DynError> {
    setup().await?;
    let [primary, listed] = active_homeservers(2).await?.try_into().unwrap();
    let scope = Arc::new(HomeserverListScope::new(std::slice::from_ref(&listed)));
    let runner = key_based_runner(&primary, scope);

    let user_id = random_pubky_id().to_string();
    start_catch_up(&user_id, &listed).await?;
    let while_catching_up = runner.pre_run().await;
    caught_up(&user_id).await?;

    assert!(!while_catching_up?.contains(&listed.to_string()));
    Ok(())
}

/// A user let into a homeserver scope gets the records it wrote while it was out, although
/// the feed of its homeserver, read whole, moved past them: the whole-feed processor reads
/// the user's own stream from the start until it is empty.
#[tokio_shared_rt::test(shared)]
async fn test_a_user_let_in_on_a_whole_feed_homeserver_catches_up() -> Result<()> {
    let mut test = WatcherTest::setup(None)
        .await?
        .remove_event_processing()
        .await;
    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Watcher:CatchUp:User".to_string(),
        status: None,
    };
    test.create_user(&user_kp, &user).await?;
    let post = PubkyAppPost {
        content: "Watcher:CatchUp:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
        lock: None,
    };
    let (post_id, _) = test.create_post(&user_kp, &post).await?;
    let user_id = PubkyId::from(user_kp.public_key());
    UserDetails::from_pubky(user_id.clone())
        .put_to_graph()
        .await?;
    mark_stub(&user_id).await?;
    // Its homeserver was just checked, so its events are skipped as an outsider's.
    let rechecked = format!("Scope:Rechecked:{user_id}");
    let mut redis_conn = get_redis_conn().await?;
    let () = redis_conn.set_ex(&rechecked, 1, 900).await?;

    let hs = test.homeserver_id.clone();
    let scope: Arc<dyn Scope> = Arc::new(HomeserverListScope::new(std::slice::from_ref(&hs)));
    let ingestor = Arc::new(UserIngestor::with_scope(
        Vec::<PubkyId>::new(),
        scope.clone(),
    ));
    let event_handler: Arc<dyn EventHandler> = Arc::new(DefaultEventHandler::new(
        default_moderation_tests(),
        ingestor.clone(),
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
    let feed = HsEventProcessorRunner {
        limit: 1000,
        event_handler,
        shutdown_rx: tokio::sync::watch::channel(false).1,
        primary_homeserver: hs.clone(),
        scope,
        retry_scheduler,
    };
    let run = || async {
        let processor = feed
            .build(hs.as_ref())
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        processor.run().await.map_err(|e| anyhow::anyhow!("{e:?}"))
    };

    // The feed moves past the user's records while it is out.
    run().await?;
    assert!(PostDetails::get_by_id(&user_id, &post_id).await?.is_none());

    assert!(ingestor.promote(&user_id).await?);
    run().await?;
    let indexed = PostDetails::get_by_id(&user_id, &post_id).await?.is_some();
    // Its stream has nothing left now: caught up.
    run().await?;
    let still_catching_up = catching_up(&[user_id.as_ref()]).await?;
    let () = redis_conn.del(&rechecked).await?;

    assert!(indexed, "the post written while it was out is read");
    assert_eq!(still_catching_up, vec![false]);
    Ok(())
}
