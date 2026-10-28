use super::utils::user_on;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use deadpool_redis::redis::AsyncCommands;
use nexus_common::db::graph::Query;
use nexus_common::db::{exec_single_row, fetch_key_from_graph, get_redis_conn, queries};
use nexus_common::models::scope::{Scope, SeedScope};
use nexus_common::models::user::outsider::create_outsider;
use nexus_common::models::user::{
    catch_up, PkdnsHomeserverResolver, PubkyConnectorResolver, UserDetails, UserHsCursor,
    UserIngestor, SCOPE_STATE_LEFT, SCOPE_STATE_OUTSIDE,
};
use nexus_common::utils::test_utils::random_pubky_id;
use nexus_common::{ScopeConfig, WatcherConfig};
use nexus_watcher::events::handlers;
use nexus_watcher::scope::{seed_members, MAX_SEED_MEMBERS};
use nexus_watcher::service::scope_bootstrap::{
    bootstrap_seed_scope, suggest_members, FollowingPage, FollowingSource,
};
use nexus_watcher::service::scope_maintenance::{ScopeChange, ScopeMaintainer};
use pubky_app_specs::{PubkyAppUser, PubkyId};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// A seed scope's ingestor and maintainer, with `moderator` pinned, limited to `users`.
/// The interval is an hour, so only a changed follow makes a tick work members out.
fn seed_maintainer(
    seed: &PubkyId,
    hops: u8,
    moderator: &PubkyId,
    users: &[&PubkyId],
) -> (UserIngestor, ScopeMaintainer) {
    seed_maintainer_with(
        seed,
        hops,
        moderator,
        users,
        Arc::new(PubkyConnectorResolver),
    )
}

/// [`seed_maintainer`] looking homeservers up through `resolver`.
fn seed_maintainer_with(
    seed: &PubkyId,
    hops: u8,
    moderator: &PubkyId,
    users: &[&PubkyId],
    resolver: Arc<dyn PkdnsHomeserverResolver>,
) -> (UserIngestor, ScopeMaintainer) {
    let mut config = WatcherConfig::default();
    config.stack.scope = ScopeConfig::Seed {
        seed: seed.clone(),
        hops,
    };
    config.moderation_id = moderator.clone();
    config.scope_maintenance.interval_ms = 3_600_000;
    let scope: Arc<dyn Scope> = Arc::new(SeedScope::new(seed.clone(), hops));
    let ingestor = UserIngestor::new(Vec::<PubkyId>::new(), resolver).with_scope(scope);
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
/// shortest distance; a user whose profile is deleted counts too.
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
    let two_hops = seed_members(&seed_id, 2, MAX_SEED_MEMBERS).await;
    let three_hops = seed_members(&seed_id, 3, MAX_SEED_MEMBERS).await;

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
            deleted.clone(),
            also_near.clone(),
        ],
    );
    exec_single_row(delete).await?;

    let expected = HashMap::from([
        (seed.clone(), 0),
        (near.clone(), 1),
        (deleted.clone(), 1),
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

/// The moderator is pinned into the scope, but the walk doesn't start from it: who the
/// moderator follows, unreached by the seed, is let go like any other user.
#[tokio_shared_rt::test(shared)]
async fn test_the_moderators_follows_are_not_members() -> Result<()> {
    let test = WatcherTest::setup(None).await?;
    let hs = test.homeserver_id.to_public_key();
    let (_, seed) = user_on(&test, &hs).await?;
    let (_, moderator) = user_on(&test, &hs).await?;
    let (_, followee) = user_on(&test, &hs).await?;
    let (ingestor, maintainer) =
        seed_maintainer(&seed, 1, &moderator, &[&seed, &moderator, &followee]);

    // Indexed under "everything", before the seed scope: both in scope.
    let everything = UserIngestor::new(Vec::<PubkyId>::new(), Arc::new(PubkyConnectorResolver));
    everything.maybe_ingest_user(&moderator).await?;
    everything.maybe_ingest_user(&followee).await?;
    ingestor.maybe_ingest_user(&seed).await?;
    follow(&moderator, &followee, &ingestor).await?;

    let at_start = maintainer.at_start().await?;
    let members = seed_members(&seed, 1, MAX_SEED_MEMBERS).await?;
    let (moderator_state, followee_state) = (
        scope_state(&moderator).await?,
        scope_state(&followee).await?,
    );

    assert_eq!(at_start, ScopeChange { joined: 0, left: 1 });
    assert_eq!(moderator_state, None, "the moderator stays in");
    assert!(
        !members.contains_key(followee.as_ref()),
        "not reached from the seed"
    );
    assert_eq!(
        followee_state.as_deref(),
        Some(SCOPE_STATE_LEFT),
        "the moderator's followee is let go"
    );
    Ok(())
}

/// A member whose profile is deleted stays in through a recompute, bound and read, so a
/// profile it writes later (read in a later batch of its stream) brings it back: it is
/// never let go between the two.
#[tokio_shared_rt::test(shared)]
async fn test_a_member_whose_profile_is_deleted_is_still_read() -> Result<()> {
    let test = WatcherTest::setup(None).await?;
    let hs = test.homeserver_id.to_public_key();
    let (_, seed) = user_on(&test, &hs).await?;
    let (_, friend) = user_on(&test, &hs).await?;
    let moderator = random_pubky_id();
    let (ingestor, maintainer) = seed_maintainer(&seed, 1, &moderator, &[&seed, &friend]);
    ingestor.maybe_ingest_user(&seed).await?;
    follow(&seed, &friend, &ingestor).await?;
    maintainer.at_start().await?;

    let profile_uri = format!("pubky://{friend}/pub/pubky.app/profile.json");
    handlers::user::del(friend.clone()).await?;
    let recomputed = maintainer.at_start().await?;
    let (deleted_state, deleted_cursors) = (
        scope_state(&friend).await?,
        UserHsCursor::read(&[friend.as_ref()], &test.homeserver_id).await?,
    );
    let profile = PubkyAppUser {
        name: "Back".to_string(),
        ..Default::default()
    };
    handlers::user::sync_put(profile, profile_uri, friend.clone()).await?;
    let members = seed_members(&seed, 1, MAX_SEED_MEMBERS).await?;
    catch_up::caught_up(&friend).await?;

    assert_eq!(recomputed, ScopeChange::default(), "nobody left");
    assert_eq!(
        deleted_state, None,
        "still in scope with its profile deleted"
    );
    assert_eq!(deleted_cursors.len(), 1, "still bound, so still read");
    assert!(members.contains_key(friend.as_ref()), "a member once back");
    Ok(())
}

/// One member following everyone can't make a seed scope index everything: past the most
/// members, the closest are kept, by id within a hop, the same every time.
#[tokio_shared_rt::test(shared)]
async fn test_seed_members_stop_at_the_most() -> Result<()> {
    WatcherTest::setup(None).await?;
    let seed = random_pubky_id().to_string();
    let mut followees = [(); 3].map(|_| random_pubky_id().to_string());
    let further = random_pubky_id().to_string();
    let create = Query::new(
        "test_seed_follows_many",
        "CREATE (s:User {id: $seed}), (f:User {id: $further})
         WITH s, f
         UNWIND $followees AS followee
         CREATE (s)-[:FOLLOWS]->(v:User {id: followee})-[:FOLLOWS]->(f)",
    )
    .param("seed", seed.clone())
    .param("further", further.clone())
    .param("followees", followees.to_vec());
    exec_single_row(create).await?;

    let seed_id = PubkyId::try_from(seed.as_str()).map_err(anyhow::Error::msg)?;
    let capped = seed_members(&seed_id, 2, 3).await;
    let again = seed_members(&seed_id, 2, 3).await;

    let mut ids = followees.to_vec();
    ids.extend([seed.clone(), further]);
    let delete = Query::new(
        "test_delete_seed_follows_many",
        "MATCH (u:User) WHERE u.id IN $ids DETACH DELETE u",
    )
    .param("ids", ids);
    exec_single_row(delete).await?;

    followees.sort();
    let expected = HashMap::from([
        (seed, 0),
        (followees[0].clone(), 1),
        (followees[1].clone(), 1),
    ]);
    assert_eq!(
        capped?, expected,
        "the seed and its two follows first by id"
    );
    assert_eq!(again?, expected, "the same every time");
    Ok(())
}

/// Looks homeservers up as the real resolver does, counting the lookups of one user.
struct CountingResolver {
    counted: PubkyId,
    lookups: AtomicU32,
}

#[async_trait::async_trait]
impl PkdnsHomeserverResolver for CountingResolver {
    async fn resolve_homeserver(
        &self,
        user_pk: &pubky::PublicKey,
    ) -> nexus_common::db::PubkyClientResult<Option<PubkyId>> {
        if user_pk.to_z32() == self.counted.as_ref() {
            self.lookups.fetch_add(1, Ordering::Relaxed);
        }
        PubkyConnectorResolver.resolve_homeserver(user_pk).await
    }
}

/// A member that couldn't join isn't looked up again by every recompute, which runs
/// within seconds of any follow, but once the interval passed.
#[tokio_shared_rt::test(shared)]
async fn test_a_member_that_couldnt_join_waits_for_the_interval() -> Result<()> {
    let test = WatcherTest::setup(None).await?;
    let hs = test.homeserver_id.to_public_key();
    let (_, seed) = user_on(&test, &hs).await?;
    let (_, friend) = user_on(&test, &hs).await?;
    let unpublished = random_pubky_id();
    let resolver = Arc::new(CountingResolver {
        counted: unpublished.clone(),
        lookups: AtomicU32::new(0),
    });
    let (ingestor, maintainer) = seed_maintainer_with(
        &seed,
        1,
        &random_pubky_id(),
        &[&seed, &unpublished, &friend],
        resolver.clone(),
    );
    ingestor.maybe_ingest_user(&seed).await?;
    create_outsider(&UserDetails::from_pubky(unpublished.clone())).await?;
    follow(&seed, &unpublished, &ingestor).await?;

    maintainer.at_start().await?;
    let first = resolver.lookups.load(Ordering::Relaxed);
    // Another follow: the next check works the members out again.
    follow(&seed, &friend, &ingestor).await?;
    let next = maintainer.tick().await?;
    let second = resolver.lookups.load(Ordering::Relaxed);
    catch_up::caught_up(&friend).await?;

    assert!(first > 0, "looked up once");
    assert_eq!(next.joined, 1, "the check ran: the new follow joined");
    assert_eq!(second, first, "not looked up again before the interval");
    Ok(())
}

/// A big Nexus answering from memory: `seed` follows `followees`, and nobody else follows
/// anyone. `None` fails every request.
struct FakeBigNexus {
    seed: String,
    followees: Option<Vec<String>>,
    requests: AtomicU32,
}

impl FakeBigNexus {
    fn new(seed: &PubkyId, followees: Option<Vec<String>>) -> Self {
        Self {
            seed: seed.to_string(),
            followees,
            requests: AtomicU32::new(0),
        }
    }
}

#[async_trait::async_trait]
impl FollowingSource for FakeBigNexus {
    async fn following(
        &self,
        user_id: &str,
        skip: usize,
    ) -> Result<FollowingPage, nexus_common::types::DynError> {
        self.requests.fetch_add(1, Ordering::Relaxed);
        let followees = self.followees.as_ref().ok_or("the big Nexus is down")?;
        Ok(FollowingPage::Followees(
            if user_id == self.seed && skip == 0 {
                followees.clone()
            } else {
                Vec::new()
            },
        ))
    }
}

/// Removes what a bootstrap of `seed` stored in Redis.
async fn forget_bootstrap(seed: &PubkyId, hops: u8) -> Result<()> {
    let mut redis_conn = get_redis_conn().await?;
    let _: () = redis_conn
        .del(&[
            format!("Scope:Suggested:{seed}:{hops}"),
            format!("Scope:Bootstrapped:{seed}:{hops}"),
        ])
        .await?;
    Ok(())
}

/// A bootstrap lets in at once the users a big Nexus says the seed follows, before any of
/// the seed's follows is read, and a later start doesn't ask again.
#[tokio_shared_rt::test(shared)]
async fn test_a_bootstrap_lets_the_suggested_members_in_at_once() -> Result<()> {
    let test = WatcherTest::setup(None).await?;
    let hs = test.homeserver_id.to_public_key();
    let (_, seed) = user_on(&test, &hs).await?;
    let (_, friend) = user_on(&test, &hs).await?;
    let moderator = random_pubky_id();
    let (ingestor, maintainer) = seed_maintainer(&seed, 1, &moderator, &[&seed, &friend]);
    ingestor.maybe_ingest_user(&seed).await?;
    maintainer.at_start().await?;
    let known_before = UserDetails::get_by_id(&friend).await?.is_some();

    let big_nexus = FakeBigNexus::new(&seed, Some(vec![friend.to_string()]));
    bootstrap_seed_scope(&maintainer, &big_nexus).await;
    let after = scope_state(&friend).await?;
    let cursors = UserHsCursor::read(&[friend.as_ref()], &test.homeserver_id).await?;
    let again = FakeBigNexus::new(&seed, Some(vec![friend.to_string()]));
    bootstrap_seed_scope(&maintainer, &again).await;

    catch_up::caught_up(&friend).await?;
    forget_bootstrap(&seed, 1).await?;
    assert!(
        !known_before,
        "unknown before: none of the seed's follows is read"
    );
    assert!(
        UserDetails::get_by_id(&friend).await?.is_some(),
        "known once suggested"
    );
    assert_eq!(after, None, "in scope at once");
    assert_eq!(cursors, vec![0], "read from the start");
    assert_eq!(again.requests.load(Ordering::Relaxed), 0, "not asked again");
    Ok(())
}

/// A suggestion the graph never confirms lets its user go once it expires; one the
/// seed's own follows confirm keeps it.
#[tokio_shared_rt::test(shared)]
async fn test_an_unconfirmed_suggestion_is_let_go_once_it_expires() -> Result<()> {
    let test = WatcherTest::setup(None).await?;
    let hs = test.homeserver_id.to_public_key();
    let (_, seed) = user_on(&test, &hs).await?;
    let (_, suggested) = user_on(&test, &hs).await?;
    let (_, confirmed) = user_on(&test, &hs).await?;
    let moderator = random_pubky_id();
    let (ingestor, maintainer) =
        seed_maintainer(&seed, 1, &moderator, &[&seed, &suggested, &confirmed]);
    ingestor.maybe_ingest_user(&seed).await?;
    follow(&seed, &confirmed, &ingestor).await?;
    let both = [suggested.to_string(), confirmed.to_string()];
    suggest_members(&seed, 1, &both).await?;
    let admitted = maintainer.at_start().await?;

    // The suggestions run out.
    let past = chrono::Utc::now().timestamp_millis() - 1;
    let mut redis_conn = get_redis_conn().await?;
    let key = format!("Scope:Suggested:{seed}:1");
    let _: () = redis_conn
        .zadd_multiple(&key, &[(past, both[0].as_str()), (past, both[1].as_str())])
        .await?;
    let expired = maintainer.recompute_now().await?;
    let (suggested_state, confirmed_state) = (
        scope_state(&suggested).await?,
        scope_state(&confirmed).await?,
    );

    catch_up::caught_up(&suggested).await?;
    catch_up::caught_up(&confirmed).await?;
    forget_bootstrap(&seed, 1).await?;
    assert_eq!(admitted, ScopeChange { joined: 2, left: 0 });
    assert_eq!(expired, ScopeChange { joined: 0, left: 1 });
    assert_eq!(
        suggested_state.as_deref(),
        Some(SCOPE_STATE_LEFT),
        "unconfirmed"
    );
    assert_eq!(confirmed_state, None, "the seed follows it");
    Ok(())
}

/// A big Nexus that can't be read suggests nobody and isn't recorded as done, so the next
/// start asks again; crawling finds the members meanwhile.
#[tokio_shared_rt::test(shared)]
async fn test_a_failed_bootstrap_is_tried_again_next_start() -> Result<()> {
    WatcherTest::setup(None).await?;
    let seed = random_pubky_id();
    let (_, maintainer) = seed_maintainer(&seed, 1, &random_pubky_id(), &[&seed]);

    let down = FakeBigNexus::new(&seed, None);
    bootstrap_seed_scope(&maintainer, &down).await;
    let mut redis_conn = get_redis_conn().await?;
    let suggested: u64 = redis_conn
        .zcard(format!("Scope:Suggested:{seed}:1"))
        .await?;
    let recorded: bool = redis_conn
        .exists(format!("Scope:Bootstrapped:{seed}:1"))
        .await?;

    assert!(down.requests.load(Ordering::Relaxed) > 0, "asked");
    assert_eq!(suggested, 0, "nothing suggested");
    assert!(!recorded, "not recorded, so tried again");
    Ok(())
}

/// A big Nexus that knows nobody for the seed (it may not know the seed yet) leaves the
/// bootstrap to be tried again next start.
#[tokio_shared_rt::test(shared)]
async fn test_a_bootstrap_that_suggests_nobody_is_tried_again() -> Result<()> {
    WatcherTest::setup(None).await?;
    let seed = random_pubky_id();
    let (_, maintainer) = seed_maintainer(&seed, 1, &random_pubky_id(), &[&seed]);

    bootstrap_seed_scope(&maintainer, &FakeBigNexus::new(&seed, Some(Vec::new()))).await;
    let mut redis_conn = get_redis_conn().await?;
    let recorded: bool = redis_conn
        .exists(format!("Scope:Bootstrapped:{seed}:1"))
        .await?;

    assert!(!recorded, "not recorded, so asked again");
    Ok(())
}
