//! Which users this Nexus indexes.
//!
//! The watcher asks one [`Scope`], built from [`ScopeConfig`], at every point where
//! it decides whose content to read: which homeservers it reads as a whole feed,
//! whether a newly discovered user is indexed or kept as a stub, and whether an
//! event's author is indexed.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use pubky_app_specs::PubkyId;

use crate::config::{ScopeConfig, WatcherConfig};
use crate::db::kv::{RedisError, RedisResult};
use crate::db::{fetch_key_from_graph, fetch_row_from_graph, get_redis_conn, queries};
use crate::models::error::ModelResult;

/// The answers every scope gives the watcher.
#[async_trait::async_trait]
pub trait Scope: Send + Sync + std::fmt::Debug {
    /// The settings this scope was built from, as `/v0/info` publishes them.
    fn config(&self) -> &ScopeConfig;

    /// Whether the user's own content is indexed.
    ///
    /// `hs_id` is the user's homeserver when the caller knows it; otherwise the scope
    /// looks up the homeserver the user is bound to, if it needs one.
    async fn is_in_scope(&self, user_id: &str, hs_id: Option<&str>) -> ModelResult<bool>;

    /// Homeservers whose whole `/events` feed is read. Users on any other homeserver
    /// are read one by one through their own event stream, if they are in scope.
    fn full_feed_homeservers(&self, primary: &PubkyId) -> Vec<PubkyId>;

    /// Runs once when the watcher starts, before any event is read.
    async fn prepare(&self) -> ModelResult<()> {
        Ok(())
    }

    /// Called after a follow or unfollow by `follower` is indexed. A seed scope asks
    /// for its members to be worked out again when the follower is one of them.
    async fn on_follow_changed(&self, _follower: &str) -> ModelResult<()> {
        Ok(())
    }
}

/// Builds the scope the configuration describes.
pub fn scope_from_config(config: &ScopeConfig) -> Arc<dyn Scope> {
    match config {
        ScopeConfig::Everything => Arc::new(EverythingScope),
        ScopeConfig::Homeservers { homeservers } => Arc::new(HomeserverListScope::new(homeservers)),
        ScopeConfig::Seed { seed, hops } => Arc::new(SeedHopsScope::new(seed.clone(), *hops)),
    }
}

/// A scope that also always includes some users, whatever the setting: the moderator,
/// whose tags take content down, must be read on every Nexus. Pinned users never
/// widen the scope themselves; who they follow doesn't matter.
#[derive(Debug)]
pub struct PinnedScope {
    inner: Arc<dyn Scope>,
    pinned: HashSet<String>,
}

impl PinnedScope {
    pub fn new(inner: Arc<dyn Scope>, pinned: impl IntoIterator<Item = PubkyId>) -> Self {
        Self {
            inner,
            pinned: pinned.into_iter().map(|id| id.to_string()).collect(),
        }
    }
}

#[async_trait::async_trait]
impl Scope for PinnedScope {
    fn config(&self) -> &ScopeConfig {
        self.inner.config()
    }

    async fn is_in_scope(&self, user_id: &str, hs_id: Option<&str>) -> ModelResult<bool> {
        if self.pinned.contains(user_id) {
            return Ok(true);
        }
        self.inner.is_in_scope(user_id, hs_id).await
    }

    fn full_feed_homeservers(&self, primary: &PubkyId) -> Vec<PubkyId> {
        self.inner.full_feed_homeservers(primary)
    }

    async fn prepare(&self) -> ModelResult<()> {
        self.inner.prepare().await
    }

    async fn on_follow_changed(&self, follower: &str) -> ModelResult<()> {
        self.inner.on_follow_changed(follower).await
    }
}

/// The watcher's scope: the configured one, with the moderator always in it.
pub fn watcher_scope(config: &WatcherConfig) -> Arc<dyn Scope> {
    Arc::new(PinnedScope::new(
        scope_from_config(&config.stack.scope),
        [config.moderation_id.clone()],
    ))
}

/// Every user Nexus can discover: the primary homeserver's feed is read whole and
/// everyone else is read through their own stream, as before scopes existed.
#[derive(Debug)]
pub struct EverythingScope;

#[async_trait::async_trait]
impl Scope for EverythingScope {
    fn config(&self) -> &ScopeConfig {
        &ScopeConfig::Everything
    }

    async fn is_in_scope(&self, _user_id: &str, _hs_id: Option<&str>) -> ModelResult<bool> {
        Ok(true)
    }

    fn full_feed_homeservers(&self, primary: &PubkyId) -> Vec<PubkyId> {
        vec![primary.clone()]
    }
}

/// Users hosted on a fixed list of homeservers, each read through its whole feed.
#[derive(Debug)]
pub struct HomeserverListScope {
    config: ScopeConfig,
    homeservers: Vec<PubkyId>,
    hs_ids: HashSet<String>,
}

impl HomeserverListScope {
    pub fn new(homeservers: &[PubkyId]) -> Self {
        Self {
            config: ScopeConfig::Homeservers {
                homeservers: homeservers.to_vec(),
            },
            homeservers: homeservers.to_vec(),
            hs_ids: homeservers.iter().map(ToString::to_string).collect(),
        }
    }
}

#[async_trait::async_trait]
impl Scope for HomeserverListScope {
    fn config(&self) -> &ScopeConfig {
        &self.config
    }

    /// In scope if the user's homeserver is listed. Without a homeserver from the
    /// caller, the graph decides: a user kept out (a stub, or one that left) is out; one
    /// bound to a homeserver is in if that homeserver is listed; one not bound yet is in,
    /// since the only feeds this scope reads are its listed homeservers'.
    async fn is_in_scope(&self, user_id: &str, hs_id: Option<&str>) -> ModelResult<bool> {
        if let Some(hs_id) = hs_id {
            return Ok(self.hs_ids.contains(hs_id));
        }
        let query = queries::get::get_user_scope_and_homeserver(user_id);
        let Some(row) = fetch_row_from_graph(query).await? else {
            return Ok(true);
        };
        let scope_state: Option<String> = row.get("scope_state")?;
        let bound_to: Option<String> = row.get("homeserver_id")?;
        Ok(match (scope_state, bound_to) {
            (Some(_), _) => false,
            (None, Some(bound_to)) => self.hs_ids.contains(&bound_to),
            (None, None) => true,
        })
    }

    fn full_feed_homeservers(&self, _primary: &PubkyId) -> Vec<PubkyId> {
        self.homeservers.clone()
    }
}

/// Prefix of a seed scope's member set: a Redis sorted set of the members, scored by
/// their hop distance from the seed.
const MEMBERS_PREFIX: &str = "Scope:Members";
/// Prefix of the flag a member's follow or unfollow sets, asking for a recompute.
const STALE_PREFIX: &str = "Scope:Stale";

/// The seed user and everyone it reaches by following, up to `hops` follows away.
///
/// Members are worked out from the graph, which holds every follow every member made,
/// whatever order they were read in, so the same follows always give the same members.
/// The result is cached in Redis (`Scope:Members:<seed>`) for the per-event check, and
/// worked out again at start, after members' follows change, and periodically. No
/// homeserver is read whole: each member is read from its own stream.
#[derive(Debug)]
pub struct SeedHopsScope {
    config: ScopeConfig,
    seed: PubkyId,
    hops: u8,
    members_key: String,
    stale_key: String,
}

impl SeedHopsScope {
    pub fn new(seed: PubkyId, hops: u8) -> Self {
        Self {
            config: ScopeConfig::Seed {
                seed: seed.clone(),
                hops,
            },
            members_key: format!("{MEMBERS_PREFIX}:{seed}"),
            stale_key: format!("{STALE_PREFIX}:{seed}"),
            seed,
            hops,
        }
    }

    pub fn seed(&self) -> &PubkyId {
        &self.seed
    }

    pub fn hops(&self) -> u8 {
        self.hops
    }

    /// The members according to the graph: the seed at hop 0 and everyone it reaches
    /// in at most `hops` follows, each at its shortest distance. One query per hop.
    pub async fn members_from_graph(&self) -> ModelResult<HashMap<String, u8>> {
        let mut members = HashMap::from([(self.seed.to_string(), 0)]);
        let mut frontier = vec![self.seed.to_string()];

        for hop in 1..=self.hops {
            if frontier.is_empty() {
                break;
            }
            let query = queries::get::get_followees_of(&frontier);
            let followees: Vec<String> = fetch_key_from_graph(query, "user_ids")
                .await?
                .unwrap_or_default();
            frontier = followees
                .into_iter()
                .filter(|followee| !members.contains_key(followee))
                .collect();
            for followee in &frontier {
                members.insert(followee.clone(), hop);
            }
        }
        Ok(members)
    }

    /// The members cached in Redis, or `None` if there is no cache (never worked out,
    /// or lost with Redis).
    pub async fn cached_members(&self) -> RedisResult<Option<HashSet<String>>> {
        let mut redis_conn = get_redis_conn().await?;
        let (exists, members): (bool, Vec<String>) = redis::pipe()
            .exists(&self.members_key)
            .zrange(&self.members_key, 0, -1)
            .query_async(&mut redis_conn)
            .await?;
        Ok(exists.then(|| members.into_iter().collect()))
    }

    /// Replaces the cached members in one step.
    pub async fn cache_members(&self, members: &HashMap<String, u8>) -> RedisResult<()> {
        let staging_key = format!("{}:Staging", self.members_key);
        let mut pipe = redis::pipe();
        pipe.atomic().del(&staging_key).ignore();
        let scored: Vec<(u8, &String)> = members.iter().map(|(id, hop)| (*hop, id)).collect();
        for chunk in scored.chunks(1_000) {
            pipe.zadd_multiple(&staging_key, chunk).ignore();
        }
        pipe.rename(&staging_key, &self.members_key).ignore();
        let mut redis_conn = get_redis_conn().await?;
        let () = pipe.query_async(&mut redis_conn).await?;
        Ok(())
    }

    /// Clears and returns the flag a member's follow change sets.
    pub async fn take_stale(&self) -> RedisResult<bool> {
        let mut redis_conn = get_redis_conn().await?;
        let flag: Option<String> = redis::cmd("GETDEL")
            .arg(&self.stale_key)
            .query_async(&mut redis_conn)
            .await?;
        Ok(flag.is_some())
    }
}

#[async_trait::async_trait]
impl Scope for SeedHopsScope {
    fn config(&self) -> &ScopeConfig {
        &self.config
    }

    async fn is_in_scope(&self, user_id: &str, _hs_id: Option<&str>) -> ModelResult<bool> {
        if user_id == self.seed.as_ref() {
            return Ok(true);
        }
        let mut redis_conn = get_redis_conn().await?;
        let hop: Option<f64> = redis::cmd("ZSCORE")
            .arg(&self.members_key)
            .arg(user_id)
            .query_async(&mut redis_conn)
            .await
            .map_err(RedisError::from)?;
        Ok(hop.is_some())
    }

    fn full_feed_homeservers(&self, _primary: &PubkyId) -> Vec<PubkyId> {
        Vec::new()
    }

    async fn on_follow_changed(&self, follower: &str) -> ModelResult<()> {
        if self.is_in_scope(follower, None).await? {
            let mut redis_conn = get_redis_conn().await?;
            let () = redis::cmd("SET")
                .arg(&self.stale_key)
                .arg(1)
                .query_async(&mut redis_conn)
                .await
                .map_err(RedisError::from)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HS_A: &str = "8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty";
    const HS_B: &str = "1hb71xx9km3f4pw5izsy1gn19ff1uuuqonw4mcygzobwkryujoiy";

    fn pk(id: &str) -> PubkyId {
        PubkyId::try_from(id).unwrap()
    }

    #[tokio::test]
    async fn everything_reads_the_primary_feed_and_indexes_everyone() {
        let scope = scope_from_config(&ScopeConfig::Everything);

        assert_eq!(scope.full_feed_homeservers(&pk(HS_A)), vec![pk(HS_A)]);
        assert!(scope.is_in_scope("anyone", None).await.unwrap());
        assert!(scope.is_in_scope("anyone", Some(HS_B)).await.unwrap());
    }

    #[tokio::test]
    async fn homeserver_list_reads_its_feeds_and_indexes_their_users() {
        let scope = scope_from_config(&ScopeConfig::Homeservers {
            homeservers: vec![pk(HS_B)],
        });

        // The primary homeserver is replaced by the list.
        assert_eq!(scope.full_feed_homeservers(&pk(HS_A)), vec![pk(HS_B)]);
        assert!(scope.is_in_scope("user", Some(HS_B)).await.unwrap());
        assert!(!scope.is_in_scope("user", Some(HS_A)).await.unwrap());
    }

    /// The moderator is in scope whatever the setting, so its takedowns are read.
    #[tokio::test]
    async fn pinned_users_are_always_in_scope() {
        let moderator = pk(HS_A);
        let homeservers: Arc<dyn Scope> = Arc::new(HomeserverListScope::new(&[pk(HS_B)]));
        let scope = PinnedScope::new(homeservers, [moderator.clone()]);

        assert!(scope.is_in_scope(&moderator, Some(HS_A)).await.unwrap());
        assert!(!scope.is_in_scope("someone", Some(HS_A)).await.unwrap());
        assert_eq!(scope.full_feed_homeservers(&pk(HS_A)), vec![pk(HS_B)]);
    }

    /// Without a homeserver from the caller, the user's binding decides.
    #[tokio_shared_rt::test(shared)]
    async fn homeserver_list_looks_up_the_binding_when_not_told(
    ) -> Result<(), crate::types::DynError> {
        use crate::models::traits::Collection;

        crate::StackManager::setup(&crate::StackConfig::default()).await?;
        let scope = HomeserverListScope::new(&[pk(HS_B)]);

        let unbound = PubkyId::from(pubky::Keypair::random().public_key());
        assert!(
            scope.is_in_scope(&unbound, None).await?,
            "unbound: from a listed feed"
        );

        for (hs, in_scope) in [(HS_B, true), (HS_A, false)] {
            let user = PubkyId::from(pubky::Keypair::random().public_key());
            crate::models::user::UserDetails::from_pubky(user.clone())
                .put_to_graph()
                .await?;
            crate::models::user::set_user_homeserver(&user, hs).await?;
            assert_eq!(
                scope.is_in_scope(&user, None).await?,
                in_scope,
                "bound to {hs}"
            );
        }
        Ok(())
    }
}
