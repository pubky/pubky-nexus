use super::utils::user_on;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::db::graph::Query;
use nexus_common::db::{exec_single_row, fetch_key_from_graph, queries};
use nexus_common::models::scope::{Scope, SeedScope};
use nexus_common::models::user::outsider::create_outsider;
use nexus_common::models::user::{
    catch_up, PubkyConnectorResolver, UserDetails, UserHsCursor, UserIngestor, SCOPE_STATE_LEFT,
    SCOPE_STATE_OUTSIDE,
};
use nexus_common::utils::test_utils::random_pubky_id;
use nexus_common::{ScopeConfig, WatcherConfig};
use nexus_watcher::events::handlers;
use nexus_watcher::scope::seed_members;
use nexus_watcher::service::scope_maintenance::{ScopeChange, ScopeMaintainer};
use pubky_app_specs::PubkyId;
use std::collections::HashMap;
use std::sync::Arc;

/// A seed scope's ingestor and maintainer, with `moderator` pinned, limited to `users`.
/// The interval is an hour, so only a changed follow makes a tick work members out.
fn seed_maintainer(
    seed: &PubkyId,
    hops: u8,
    moderator: &PubkyId,
    users: &[&PubkyId],
) -> (UserIngestor, ScopeMaintainer) {
    let mut config = WatcherConfig::default();
    config.stack.scope = ScopeConfig::Seed {
        seed: seed.clone(),
        hops,
    };
    config.moderation_id = moderator.clone();
    config.scope_maintenance.interval_ms = 3_600_000;
    let scope: Arc<dyn Scope> = Arc::new(SeedScope::new(seed.clone(), hops));
    let ingestor = UserIngestor::new(Vec::<PubkyId>::new(), Arc::new(PubkyConnectorResolver))
        .with_scope(scope);
    let limit_to = Some(users.iter().map(|id| id.to_string()).collect());
    let maintainer = ScopeMaintainer::new(&config, ingestor.clone(), limit_to);
    (ingestor, maintainer)
}

/// The scope's state of `user_id` in the graph: `None` when in scope.
async fn scope_state(user_id: &str) -> Result<Option<String>> {
    let query = queries::get::get_user_scope_state(user_id);
    let state: Option<Option<String>> = fetch_key_from_graph(query, "scope_state").await?;
    Ok(state.flatten())
}

/// `follower` follows `followee`, as an event would index it: a followee never met is
/// ingested by the first attempt, and the follow lands on the retry.
async fn follow(follower: &PubkyId, followee: &PubkyId, ingestor: &UserIngestor) -> Result<()> {
    let first = handlers::follow::sync_put(follower.clone(), followee.clone(), ingestor).await;
    if first.is_err() {
        handlers::follow::sync_put(follower.clone(), followee.clone(), ingestor).await?;
    }
    Ok(())
}

/// The members are the seed and who it reaches in up to `hops` follows, each at its
/// shortest distance; a deleted user is left out.
#[tokio_shared_rt::test(shared)]
async fn test_seed_members_are_who_the_seed_reaches_by_following() -> Result<()> {
    WatcherTest::setup(None).await?;
    let [seed, near, far, farther, deleted, also_near] =
        [(); 6].map(|_| random_pubky_id().to_string());
    let create = Query::new(
        "test_seed_follows",
        "CREATE (s:User {id: $seed}), (n:User {id: $near}), (f:User {id: $far}),
                (r:User {id: $farther}), (d:User {id: $deleted, deleted: true}),
                (a:User {id: $also_near}),
                (s)-[:FOLLOWS]->(n), (n)-[:FOLLOWS]->(f), (f)-[:FOLLOWS]->(r),
                (s)-[:FOLLOWS]->(d), (n)-[:FOLLOWS]->(s),
                (s)-[:FOLLOWS]->(a), (n)-[:FOLLOWS]->(a)",
    )
    .param("seed", seed.clone())
    .param("near", near.clone())
    .param("far", far.clone())
    .param("farther", farther.clone())
    .param("deleted", deleted.clone())
    .param("also_near", also_near.clone());
    exec_single_row(create).await?;

    let seed_id = PubkyId::try_from(seed.as_str()).map_err(anyhow::Error::msg)?;
    let two_hops = seed_members(&seed_id, 2).await;
    let three_hops = seed_members(&seed_id, 3).await;

    let delete = Query::new(
        "test_delete_seed_follows",
        "MATCH (u:User) WHERE u.id IN $ids DETACH DELETE u",
    )
    .param(
        "ids",
        vec![
            seed.clone(),
            near.clone(),
            far.clone(),
            farther.clone(),
            deleted,
            also_near.clone(),
        ],
    );
    exec_single_row(delete).await?;

    let expected = HashMap::from([
        (seed.clone(), 0),
        (near.clone(), 1),
        (also_near.clone(), 1),
        (far.clone(), 2),
    ]);
    assert_eq!(
        two_hops?, expected,
        "two hops, each at its shortest distance"
    );
    let mut expected = expected;
    expected.insert(farther, 3);
    assert_eq!(three_hops?, expected, "one more hop reaches one more user");
    Ok(())
}

/// At start, a seed scope narrowed from "everything" lets in who the seed follows and
/// lets go of who it doesn't reach; when the seed unfollows, the next check lets that
/// member go too, without waiting for the interval.
#[tokio_shared_rt::test(shared)]
async fn test_a_seed_scope_follows_the_seeds_follows() -> Result<()> {
    let test = WatcherTest::setup(None).await?;
    let hs = test.homeserver_id.to_public_key();
    let (_, seed) = user_on(&test, &hs).await?;
    let (_, friend) = user_on(&test, &hs).await?;
    let (_, stranger) = user_on(&test, &hs).await?;
    let moderator = random_pubky_id();
    let (ingestor, maintainer) =
        seed_maintainer(&seed, 1, &moderator, &[&seed, &friend, &stranger]);

    // Indexed under "everything", before the seed scope: in scope, bound and polled.
    UserIngestor::new(Vec::<PubkyId>::new(), Arc::new(PubkyConnectorResolver))
        .maybe_ingest_user(&stranger)
        .await?;
    ingestor.maybe_ingest_user(&seed).await?;
    follow(&seed, &friend, &ingestor).await?;
    assert_eq!(
        scope_state(&friend).await?.as_deref(),
        Some(SCOPE_STATE_OUTSIDE),
        "met through a follow: out until the members are worked out"
    );

    let at_start = maintainer.at_start().await?;
    let friend_cursors = UserHsCursor::read(&[friend.as_ref()], &test.homeserver_id).await?;
    let (stranger_state, friend_state) =
        (scope_state(&stranger).await?, scope_state(&friend).await?);

    handlers::follow::del(seed.clone(), friend.clone()).await?;
    let after_unfollow = maintainer.tick().await?;
    let unchanged = maintainer.tick().await?;
    let friend_after = scope_state(&friend).await?;
    catch_up::caught_up(&friend).await?;

    assert_eq!(at_start, ScopeChange { joined: 1, left: 1 });
    assert_eq!(friend_state, None, "the seed's followee joined");
    assert_eq!(friend_cursors, vec![0], "and is read from the start");
    assert_eq!(
        stranger_state.as_deref(),
        Some(SCOPE_STATE_LEFT),
        "unreached"
    );
    assert_eq!(after_unfollow, ScopeChange { joined: 0, left: 1 });
    assert_eq!(
        friend_after.as_deref(),
        Some(SCOPE_STATE_LEFT),
        "unfollowed"
    );
    assert_eq!(unchanged, ScopeChange::default(), "no follow changed since");
    Ok(())
}

/// A member that can't join (no published homeserver) doesn't stop the others, and stays
/// out to be tried again at the next run; the moderator, unreached, is never let go.
#[tokio_shared_rt::test(shared)]
async fn test_a_member_that_cant_join_doesnt_stop_the_others() -> Result<()> {
    let test = WatcherTest::setup(None).await?;
    let hs = test.homeserver_id.to_public_key();
    let (_, seed) = user_on(&test, &hs).await?;
    let (_, friend) = user_on(&test, &hs).await?;
    let (_, moderator) = user_on(&test, &hs).await?;
    // Tried before the friend (same hop, smaller id), so a failure that stopped the run
    // would keep the friend out.
    let unpublished = std::iter::repeat_with(random_pubky_id)
        .find(|id| id.as_ref() < friend.as_ref())
        .expect("a smaller id comes up");
    let (ingestor, maintainer) = seed_maintainer(
        &seed,
        1,
        &moderator,
        &[&seed, &friend, &unpublished, &moderator],
    );

    UserIngestor::new(Vec::<PubkyId>::new(), Arc::new(PubkyConnectorResolver))
        .maybe_ingest_user(&moderator)
        .await?;
    ingestor.maybe_ingest_user(&seed).await?;
    create_outsider(&UserDetails::from_pubky(unpublished.clone())).await?;
    follow(&seed, &unpublished, &ingestor).await?;
    follow(&seed, &friend, &ingestor).await?;

    let at_start = maintainer.at_start().await?;
    let unpublished_state = scope_state(&unpublished).await?;
    let (friend_state, moderator_state) =
        (scope_state(&friend).await?, scope_state(&moderator).await?);
    catch_up::caught_up(&friend).await?;

    assert_eq!(at_start, ScopeChange { joined: 1, left: 0 });
    assert_eq!(friend_state, None, "joined despite the other");
    assert_eq!(
        unpublished_state.as_deref(),
        Some(SCOPE_STATE_OUTSIDE),
        "out, to be tried again"
    );
    assert_eq!(moderator_state, None, "the moderator stays in");
    Ok(())
}
