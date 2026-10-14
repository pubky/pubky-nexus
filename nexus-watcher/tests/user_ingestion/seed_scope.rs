//! A seed scope's members are worked out from the graph, so they don't depend on the
//! order follows were read in, members leave when follows go, a narrower setting lets
//! users go, and losing Redis loses nothing.

use super::utils::create_external_test_homeserver;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::db::graph::Query;
use nexus_common::db::{fetch_row_from_graph, get_redis_conn};
use nexus_common::models::follow::{Followers, UserFollows};
use nexus_common::models::scope::{PinnedScope, Scope, SeedHopsScope};
use nexus_common::models::user::{is_stub, UserIngestor};
use nexus_common::types::DynError;
use nexus_common::{ScopeConfig, WatcherConfig};
use nexus_watcher::service::scope_bootstrap::{bootstrap_seed_scope, FollowingSource};
use nexus_watcher::service::scope_maintenance::{ScopeChange, ScopeMaintainer};
use pubky::Keypair;
use pubky_app_specs::PubkyId;
use std::collections::HashMap;
use std::sync::Arc;

/// Users registered on one test homeserver: `[seed, b, c, d]`.
async fn users(test: &mut WatcherTest) -> Result<[PubkyId; 4]> {
    let hs_pk = create_external_test_homeserver(test).await?;
    let mut ids = Vec::new();
    for _ in 0..4 {
        let kp = Keypair::random();
        test.register_user_in_hs(&kp, &hs_pk).await?;
        ids.push(PubkyId::from(kp.public_key()));
    }
    Ok(ids.try_into().unwrap())
}

fn config(seed: &PubkyId, hops: u8) -> WatcherConfig {
    let mut config = WatcherConfig::default();
    config.stack.scope = ScopeConfig::Seed {
        seed: seed.clone(),
        hops,
    };
    config
}

/// The watcher's ingestor and maintainer for this seed, limited to the test's users
/// because tests share one graph.
fn seed_scope(seed: &PubkyId, hops: u8, users: &[PubkyId]) -> (UserIngestor, ScopeMaintainer) {
    let config = config(seed, hops);
    let scope: Arc<dyn Scope> = Arc::new(PinnedScope::new(
        Arc::new(SeedHopsScope::new(seed.clone(), hops)),
        [config.moderation_id.clone()],
    ));
    let ingestor = UserIngestor::with_scope(Vec::<PubkyId>::new(), scope);
    let limit_to = users.iter().map(ToString::to_string).collect();
    let maintainer = ScopeMaintainer::new(&config, ingestor.clone(), Some(limit_to));
    (ingestor, maintainer)
}

async fn follow(follower: &PubkyId, followee: &PubkyId) -> Result<()> {
    Followers::put_to_graph(follower, followee).await?;
    Ok(())
}

/// Whether the user is bound to a homeserver with no scope state: polled as a member.
async fn is_polled(user_id: &str) -> Result<bool> {
    let query = Query::new(
        "test_is_polled",
        "MATCH (u:User {id: $user_id})-[:HOSTED_BY]->(:Homeserver)
         WHERE u.scope_state IS NULL
         RETURN u.id AS id",
    )
    .param("user_id", user_id.to_string());
    Ok(fetch_row_from_graph(query).await?.is_some())
}

async fn scope_state(user_id: &str) -> Result<Option<String>> {
    let query = Query::new(
        "test_scope_state",
        "MATCH (u:User {id: $user_id}) RETURN u.scope_state AS scope_state",
    )
    .param("user_id", user_id.to_string());
    let row = fetch_row_from_graph(query).await?.expect("user exists");
    Ok(row.get("scope_state")?)
}

/// Problem 2a: the same follows give the same members, whatever order they arrive in.
/// Carol's follow of Dave was read while Carol was at the edge; when Carol moves closer,
/// Dave joins, because the follow was in the graph all along.
#[tokio_shared_rt::test(shared)]
async fn test_a_member_moving_closer_brings_in_its_follows() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let [seed, b, c, d] = users(&mut test).await?;
    let (ingestor, maintainer) =
        seed_scope(&seed, 2, &[seed.clone(), b.clone(), c.clone(), d.clone()]);

    ingestor.ensure_in_scope(&seed).await?;
    for user in [&b, &c, &d] {
        ingestor.maybe_ingest_user(user).await?;
        assert!(is_stub(user).await?, "not a member yet");
    }
    follow(&seed, &b).await?;
    follow(&b, &c).await?;
    follow(&c, &d).await?;

    let change = maintainer.at_start().await?;
    assert_eq!(
        change,
        ScopeChange { joined: 2, left: 0 },
        "b and c; the seed was in"
    );
    assert!(is_polled(&c).await?, "c is at hop 2, the edge");
    assert!(is_stub(&d).await?, "d is 3 follows away");

    // The seed now follows c directly: c moves to hop 1, so d is at hop 2.
    follow(&seed, &c).await?;
    ingestor.scope().on_follow_changed(&seed).await?;
    let change = maintainer.tick().await?;

    assert_eq!(change, ScopeChange { joined: 1, left: 0 });
    assert!(is_polled(&d).await?, "d joins through c's stored follow");
    Ok(())
}

/// Problems 2b and 2c: an unfollow, or a narrower setting, lets members go: they stop
/// being polled, and nobody else is touched.
#[tokio_shared_rt::test(shared)]
async fn test_members_leave_when_follows_go_or_the_scope_narrows() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let [seed, b, c, d] = users(&mut test).await?;
    let all = [seed.clone(), b.clone(), c.clone(), d.clone()];
    let (ingestor, maintainer) = seed_scope(&seed, 2, &all);

    ingestor.ensure_in_scope(&seed).await?;
    for user in [&b, &c, &d] {
        ingestor.maybe_ingest_user(user).await?;
    }
    follow(&seed, &b).await?;
    follow(&b, &c).await?;
    follow(&seed, &d).await?;
    maintainer.at_start().await?;
    assert!(is_polled(&b).await? && is_polled(&c).await? && is_polled(&d).await?);

    // The seed unfollows b: b leaves, and so does c, which was only in through b.
    Followers::del_from_graph(&seed, &b).await?;
    ingestor.scope().on_follow_changed(&seed).await?;
    assert_eq!(maintainer.tick().await?, ScopeChange { joined: 0, left: 2 });
    for user in [&b, &c] {
        assert!(!is_polled(user).await?, "{user} is no longer polled");
        assert_eq!(scope_state(user).await?.as_deref(), Some("left"));
    }
    assert!(is_polled(&d).await?, "d still follows from the seed");

    // d follows b: b comes back at hop 2, read from the start of its stream again.
    follow(&d, &b).await?;
    ingestor.scope().on_follow_changed(&d).await?;
    assert_eq!(maintainer.tick().await?, ScopeChange { joined: 1, left: 0 });
    assert!(is_polled(&b).await?);

    // Narrowed to 1 hop: b, 2 follows away, leaves; d, 1 follow away, stays.
    let (_, narrower) = seed_scope(&seed, 1, &all);
    assert_eq!(
        narrower.at_start().await?,
        ScopeChange { joined: 0, left: 1 }
    );
    assert!(!is_polled(&b).await?, "b is 2 follows away");
    assert!(is_polled(&d).await?, "d is 1 follow away");
    Ok(())
}

/// Problem 2d: losing Redis loses no member. A member's event while the cache is gone
/// is retried rather than skipped, and the next start rebuilds the cache from the graph
/// without letting anyone go.
#[tokio_shared_rt::test(shared)]
async fn test_losing_redis_loses_no_member() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let [seed, b, c, d] = users(&mut test).await?;
    let (ingestor, maintainer) =
        seed_scope(&seed, 1, &[seed.clone(), b.clone(), c.clone(), d.clone()]);

    ingestor.ensure_in_scope(&seed).await?;
    ingestor.maybe_ingest_user(&b).await?;
    follow(&seed, &b).await?;
    maintainer.at_start().await?;
    assert!(ingestor.scope().is_in_scope(&b, None).await?);

    let mut redis_conn = get_redis_conn().await?;
    let () = deadpool_redis::redis::cmd("DEL")
        .arg(format!("Scope:Members:{seed}"))
        .query_async(&mut redis_conn)
        .await?;
    assert!(!ingestor.scope().is_in_scope(&b, None).await?, "cache lost");
    assert!(is_polled(&b).await?, "but b is still bound and polled");

    let change = maintainer.at_start().await?;
    assert_eq!(change.left, 0, "nobody leaves for a lost cache");
    assert!(
        ingestor.scope().is_in_scope(&b, None).await?,
        "rebuilt from the graph"
    );
    Ok(())
}

/// Switching back to "everything" lets every user kept out of a scope back in.
#[tokio_shared_rt::test(shared)]
async fn test_widening_to_everything_lets_everyone_back_in() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let [seed, b, c, d] = users(&mut test).await?;
    let all = [seed.clone(), b.clone(), c.clone(), d.clone()];
    let (ingestor, maintainer) = seed_scope(&seed, 1, &all);

    ingestor.ensure_in_scope(&seed).await?;
    for user in [&b, &c] {
        ingestor.maybe_ingest_user(user).await?;
    }
    follow(&seed, &b).await?;
    maintainer.at_start().await?;
    Followers::del_from_graph(&seed, &b).await?;
    ingestor.scope().on_follow_changed(&seed).await?;
    maintainer.tick().await?;
    assert_eq!(scope_state(&b).await?.as_deref(), Some("left"));
    assert!(is_stub(&c).await?);

    let mut everything = WatcherConfig::default();
    everything.stack.scope = ScopeConfig::Everything;
    let limit_to = all.iter().map(ToString::to_string).collect();
    let widened = ScopeMaintainer::new(&everything, UserIngestor::default(), Some(limit_to));
    widened.at_start().await?;

    for user in [&b, &c] {
        assert_eq!(scope_state(user).await?, None, "{user} is back in scope");
    }
    assert!(!is_stub(&c).await?);
    Ok(())
}

/// Who follows whom on a fake big Nexus; `None` fails every request.
struct FakeBigNexus(Option<HashMap<String, Vec<String>>>, std::sync::Mutex<u32>);

#[async_trait::async_trait]
impl FollowingSource for FakeBigNexus {
    async fn following(&self, user_id: &str, skip: usize) -> Result<Option<Vec<String>>, DynError> {
        *self.1.lock().unwrap() += 1;
        let graph = self.0.as_ref().ok_or("the big Nexus is down")?;
        Ok(graph
            .get(user_id)
            .map(|followees| followees.iter().skip(skip).cloned().collect()))
    }
}

/// A big Nexus's follows suggest every member at start, so all of them are polled at
/// once instead of hop by hop; the bootstrap runs once per seed and hop count.
#[tokio_shared_rt::test(shared)]
async fn test_bootstrap_admits_the_members_a_big_nexus_suggests() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let [seed, b, c, d] = users(&mut test).await?;
    let (ingestor, maintainer) =
        seed_scope(&seed, 2, &[seed.clone(), b.clone(), c.clone(), d.clone()]);
    ingestor.ensure_in_scope(&seed).await?;
    maintainer.at_start().await?;

    let big_nexus = FakeBigNexus(
        Some(HashMap::from([
            (seed.to_string(), vec![b.to_string()]),
            (b.to_string(), vec![c.to_string()]),
            (c.to_string(), vec![d.to_string()]),
        ])),
        Default::default(),
    );
    let scope = SeedHopsScope::new(seed.clone(), 2);
    bootstrap_seed_scope(&scope, &maintainer, &big_nexus).await;

    assert!(
        is_polled(&b).await? && is_polled(&c).await?,
        "suggested and polled at once"
    );
    assert!(!is_polled(&d).await?, "3 follows away");
    assert!(scope.is_bootstrapped().await?);

    let requests = *big_nexus.1.lock().unwrap();
    bootstrap_seed_scope(&scope, &maintainer, &big_nexus).await;
    assert_eq!(*big_nexus.1.lock().unwrap(), requests, "not asked again");
    Ok(())
}

/// If the big Nexus can't be read, nothing is suggested and the bootstrap isn't marked
/// done: crawling finds the members, and the next start tries again.
#[tokio_shared_rt::test(shared)]
async fn test_bootstrap_falls_back_to_crawling_when_the_big_nexus_fails() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let [seed, b, c, d] = users(&mut test).await?;
    let (_, maintainer) = seed_scope(&seed, 2, &[seed.clone(), b, c, d]);

    let scope = SeedHopsScope::new(seed.clone(), 2);
    bootstrap_seed_scope(&scope, &maintainer, &FakeBigNexus(None, Default::default())).await;

    assert!(!scope.is_bootstrapped().await?);
    Ok(())
}
