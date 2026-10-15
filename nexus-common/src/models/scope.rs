//! Which users this Nexus indexes.
//!
//! The watcher asks one [`Scope`], built from [`ScopeConfig`], at every point where
//! it decides whose content to read: which homeservers it reads as a whole feed,
//! whether a newly discovered user is indexed or kept as a stub, and whether an
//! event's author is indexed.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use deadpool_redis::redis::AsyncCommands;
use pubky_app_specs::PubkyId;
use serde::{Deserialize, Serialize};

use crate::config::{ScopeConfig, WatcherConfig};
use crate::db::kv::RedisResult;
use crate::db::{fetch_key_from_graph, fetch_row_from_graph, get_redis_conn, queries, RedisOps};
use crate::models::error::ModelResult;

/// The answers every scope gives the watcher.
#[async_trait::async_trait]
pub trait Scope: Send + Sync + std::fmt::Debug {
    /// The settings this scope was built from, as `/v0/info` publishes them.
    fn config(&self) -> &ScopeConfig;

    /// Whether the user's own content is indexed. `hs_id` is the user's homeserver when
    /// the caller knows it; otherwise the scope looks it up if it needs one.
    async fn is_in_scope(&self, user_id: &str, hs_id: Option<&str>) -> ModelResult<bool>;

    /// Homeservers whose whole `/events` feed is read. Users on any other homeserver
    /// are read one by one through their own event stream, if they are in scope.
    fn full_feed_homeservers(&self, primary: &PubkyId) -> Vec<PubkyId>;

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
}

impl HomeserverListScope {
    pub fn new(homeservers: &[PubkyId]) -> Self {
        Self {
            config: ScopeConfig::Homeservers {
                homeservers: homeservers.to_vec(),
            },
        }
    }

    fn homeservers(&self) -> &[PubkyId] {
        match &self.config {
            ScopeConfig::Homeservers { homeservers } => homeservers,
            _ => &[],
        }
    }

    fn is_listed(&self, hs_id: &str) -> bool {
        self.homeservers()
            .iter()
            .any(|listed| listed.as_ref() == hs_id)
    }
}

#[async_trait::async_trait]
impl Scope for HomeserverListScope {
    fn config(&self) -> &ScopeConfig {
        &self.config
    }

    /// Without `hs_id`, the graph decides: a user kept out (any `scope_state`) is out,
    /// a bound user is in if its homeserver is listed, and an unbound one is in, since
    /// the only feeds this scope reads are its listed homeservers'.
    async fn is_in_scope(&self, user_id: &str, hs_id: Option<&str>) -> ModelResult<bool> {
        if let Some(hs_id) = hs_id {
            return Ok(self.is_listed(hs_id));
        }
        let query = queries::get::get_user_scope_and_homeserver(user_id);
        let Some(row) = fetch_row_from_graph(query).await? else {
            return Ok(true);
        };
        let scope_state: Option<String> = row.get("scope_state")?;
        let bound_to: Option<String> = row.get("homeserver_id")?;
        Ok(match (scope_state, bound_to) {
            (Some(_), _) => false,
            (None, Some(bound_to)) => self.is_listed(&bound_to),
            (None, None) => true,
        })
    }

    fn full_feed_homeservers(&self, _primary: &PubkyId) -> Vec<PubkyId> {
        self.homeservers().to_vec()
    }
}

/// How long a suggested user is a member without the graph confirming it: time for its
/// followers' streams to be read, so their follows of it land in the graph.
const SUGGESTION_TTL_MS: i64 = 24 * 60 * 60 * 1_000;

/// The Redis keys of a seed scope, all under `Scope:`.
#[derive(Serialize, Deserialize)]
struct ScopeIndex;

impl RedisOps for ScopeIndex {}

/// The seed user and everyone it reaches by following, up to `hops` follows away.
///
/// Members are worked out from the graph, which holds every follow every member made,
/// whatever order they were read in, so the same follows always give the same members.
/// The result is cached in `Scope:Members:<seed>` (scored by hop) for the per-event
/// check. No homeserver is read whole: each member is read from its own stream.
#[derive(Debug)]
pub struct SeedHopsScope {
    config: ScopeConfig,
    seed: PubkyId,
    hops: u8,
}

impl SeedHopsScope {
    pub fn new(seed: PubkyId, hops: u8) -> Self {
        Self {
            config: ScopeConfig::Seed {
                seed: seed.clone(),
                hops,
            },
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

    fn key(&self, name: &str) -> String {
        format!("Scope:{name}:{}", self.seed)
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

    /// The members: those the graph gives, plus users a big Nexus suggested whose
    /// suggestion hasn't expired (counted at the last hop, as their distance is unknown).
    pub async fn members(&self) -> ModelResult<HashMap<String, u8>> {
        let mut members = self.members_from_graph().await?;
        for suggested in self.suggested_members().await? {
            members.entry(suggested).or_insert(self.hops);
        }
        Ok(members)
    }

    /// Admits users a big Nexus suggested as members, until the suggestion expires.
    pub async fn suggest_members(&self, user_ids: &[PubkyId]) -> RedisResult<()> {
        let expires_at = chrono::Utc::now().timestamp_millis() as f64 + SUGGESTION_TTL_MS as f64;
        let scored: Vec<(f64, &str)> = user_ids
            .iter()
            .map(|id| (expires_at, id.as_ref()))
            .collect();
        ScopeIndex::put_index_sorted_set(&["Suggested", &self.seed], &scored, Some("Scope"), None)
            .await
    }

    /// The suggested users whose suggestion hasn't expired; expired ones are dropped.
    async fn suggested_members(&self) -> RedisResult<Vec<String>> {
        let now = chrono::Utc::now().timestamp_millis();
        let key = self.key("Suggested");
        let mut redis_conn = get_redis_conn().await?;
        let () = redis_conn.zrembyscore(&key, "-inf", now).await?;
        Ok(redis_conn.zrangebyscore(&key, now, "+inf").await?)
    }

    /// Whether a big Nexus already suggested the members for this seed and hop count.
    pub async fn is_bootstrapped(&self) -> RedisResult<bool> {
        let mut redis_conn = get_redis_conn().await?;
        Ok(redis_conn.exists(self.bootstrapped_key()).await?)
    }

    /// Records the bootstrap, so a restart doesn't ask the big Nexus again.
    pub async fn mark_bootstrapped(&self) -> RedisResult<()> {
        let now = chrono::Utc::now().timestamp_millis();
        let mut redis_conn = get_redis_conn().await?;
        Ok(redis_conn.set(self.bootstrapped_key(), now).await?)
    }

    fn bootstrapped_key(&self) -> String {
        format!("{}:{}", self.key("Bootstrapped"), self.hops)
    }

    /// The members cached in Redis, or `None` if there is no cache (never worked out,
    /// or lost with Redis). The cache always holds the seed, so it is never empty.
    ///
    /// Read whole, not through the paged sorted-set helpers (capped at 1,000): every
    /// member has to be compared to see who left.
    pub async fn cached_members(&self) -> RedisResult<Option<HashSet<String>>> {
        let mut redis_conn = get_redis_conn().await?;
        let members: Vec<String> = redis_conn.zrange(self.key("Members"), 0, -1).await?;
        Ok((!members.is_empty()).then(|| members.into_iter().collect()))
    }

    /// Replaces the cached members atomically.
    pub async fn cache_members(&self, members: &HashMap<String, u8>) -> RedisResult<()> {
        let scored: Vec<(f64, &str)> = members
            .iter()
            .map(|(id, hop)| (f64::from(*hop), id.as_str()))
            .collect();
        ScopeIndex::replace_index_sorted_set(&["Members", &self.seed], &scored, Some("Scope"), None)
            .await
    }

    /// Drops users from the cached members, so the next recompute sees them as new and
    /// tries to bring them in again.
    pub async fn forget_members(&self, user_ids: &[String]) -> RedisResult<()> {
        let user_ids: Vec<&str> = user_ids.iter().map(String::as_str).collect();
        ScopeIndex::remove_from_index_sorted_set(Some("Scope"), &["Members", &self.seed], &user_ids)
            .await
    }

    /// Clears and returns the flag a member's follow change sets.
    pub async fn take_stale(&self) -> RedisResult<bool> {
        let mut redis_conn = get_redis_conn().await?;
        let flag: Option<String> = redis_conn.get_del(self.key("Stale")).await?;
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
        let hop = ScopeIndex::check_sorted_set_member(
            Some("Scope"),
            &["Members", &self.seed],
            &[user_id],
        )
        .await?;
        Ok(hop.is_some())
    }

    fn full_feed_homeservers(&self, _primary: &PubkyId) -> Vec<PubkyId> {
        Vec::new()
    }

    async fn on_follow_changed(&self, follower: &str) -> ModelResult<()> {
        if self.is_in_scope(follower, None).await? {
            let mut redis_conn = get_redis_conn().await?;
            let () = redis_conn
                .set(self.key("Stale"), 1)
                .await
                .map_err(crate::db::kv::RedisError::from)?;
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
            // Other suites share this graph: a new user left behind lands in their
            // "today" windows (e.g. the influencers) and shifts what they expect.
            let delete = crate::db::graph::Query::new(
                "delete_test_user",
                "MATCH (u:User {id: $id}) DETACH DELETE u",
            )
            .param("id", user.to_string());
            crate::db::exec_single_row(delete).await?;
        }
        Ok(())
    }

    /// Every cached member is read back, however many there are, so members past the
    /// first page can still be let go.
    #[tokio_shared_rt::test(shared)]
    async fn cached_members_are_read_whole() -> Result<(), crate::types::DynError> {
        crate::StackManager::setup(&crate::StackConfig::default()).await?;
        let seed = PubkyId::from(pubky::Keypair::random().public_key());
        let scope = SeedHopsScope::new(seed.clone(), 2);

        let mut members: HashMap<String, u8> =
            (0..1_500).map(|i| (format!("member{i}"), 2)).collect();
        members.insert(seed.to_string(), 0);
        scope.cache_members(&members).await?;

        let cached = scope.cached_members().await?.expect("cached");
        assert_eq!(cached.len(), 1_501);

        scope.cache_members(&HashMap::new()).await?;
        Ok(())
    }
}
