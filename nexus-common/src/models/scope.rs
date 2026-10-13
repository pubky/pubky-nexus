//! Which users this Nexus indexes.
//!
//! The watcher asks one [`Scope`], built from [`ScopeConfig`], at every point where
//! it decides whose content to read: which homeservers it reads as a whole feed,
//! whether a newly discovered user is indexed or kept as a stub, whether an event's
//! author is indexed, and whether a follow pulls the followee into the scope.

use std::collections::HashSet;
use std::sync::Arc;

use pubky_app_specs::PubkyId;

use crate::config::ScopeConfig;
use crate::db::get_redis_conn;
use crate::db::kv::RedisResult;
use crate::models::error::ModelResult;

/// Prefix of a seed scope's member set: a Redis sorted set of the members, scored
/// by their hop distance from the seed.
const MEMBERS_PREFIX: &str = "Scope:Members";

/// The answers every scope gives the watcher.
#[async_trait::async_trait]
pub trait Scope: Send + Sync + std::fmt::Debug {
    /// The settings this scope was built from, as `/v0/info` publishes them.
    fn config(&self) -> &ScopeConfig;

    /// Whether the user's own content is indexed.
    ///
    /// `hs_id` is the user's homeserver when the caller knows it. Without it, a
    /// homeserver scope answers yes: the only feeds it reads are its own homeservers'.
    async fn is_in_scope(&self, user_id: &str, hs_id: Option<&str>) -> ModelResult<bool>;

    /// Homeservers whose whole `/events` feed is read. Users on any other homeserver
    /// are read one by one through their own event stream, if they are in scope.
    fn full_feed_homeservers(&self, primary: &PubkyId) -> Vec<PubkyId>;

    /// Called before `follower`'s follow of `followee` is indexed. Returns whether
    /// the follow brought `followee` into the scope.
    async fn on_follow(&self, _follower: &str, _followee: &str) -> ModelResult<bool> {
        Ok(false)
    }

    /// Runs once when the watcher starts, before any event is read.
    async fn prepare(&self) -> ModelResult<()> {
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

    async fn is_in_scope(&self, _user_id: &str, hs_id: Option<&str>) -> ModelResult<bool> {
        Ok(hs_id.is_none_or(|hs_id| self.hs_ids.contains(hs_id)))
    }

    fn full_feed_homeservers(&self, _primary: &PubkyId) -> Vec<PubkyId> {
        self.homeservers.clone()
    }
}

/// The seed user and everyone it reaches by following, up to `hops` follows away.
///
/// Members are kept in Redis with their hop distance, so every part of the watcher
/// sees the same set. The set is keyed by the seed and the hop count, so changing
/// either one in the config starts a new set. No homeserver is read whole: each member
/// is read from the start of its own stream, which also covers users who join
/// after the watcher has started.
#[derive(Debug)]
pub struct SeedHopsScope {
    config: ScopeConfig,
    seed: PubkyId,
    hops: u8,
    members_key: String,
}

impl SeedHopsScope {
    pub fn new(seed: PubkyId, hops: u8) -> Self {
        Self {
            config: ScopeConfig::Seed {
                seed: seed.clone(),
                hops,
            },
            members_key: format!("{MEMBERS_PREFIX}:{seed}:{hops}"),
            seed,
            hops,
        }
    }

    /// The member's hop distance from the seed, or `None` if it is not a member.
    pub async fn hop_of(&self, user_id: &str) -> RedisResult<Option<u8>> {
        let mut redis_conn = get_redis_conn().await?;
        let score: Option<f64> = redis::cmd("ZSCORE")
            .arg(&self.members_key)
            .arg(user_id)
            .query_async(&mut redis_conn)
            .await?;
        Ok(score.map(|hop| hop as u8))
    }

    /// Adds the member at `hop`, or moves it closer if it is already further away.
    async fn add_member(&self, user_id: &str, hop: u8) -> RedisResult<()> {
        let mut redis_conn = get_redis_conn().await?;
        let () = redis::cmd("ZADD")
            .arg(&self.members_key)
            .arg("LT")
            .arg(hop)
            .arg(user_id)
            .query_async(&mut redis_conn)
            .await?;
        Ok(())
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
        Ok(self.hop_of(user_id).await?.is_some())
    }

    fn full_feed_homeservers(&self, _primary: &PubkyId) -> Vec<PubkyId> {
        Vec::new()
    }

    async fn on_follow(&self, follower: &str, followee: &str) -> ModelResult<bool> {
        let Some(follower_hop) = self.hop_of(follower).await? else {
            return Ok(false);
        };
        if follower_hop >= self.hops {
            return Ok(false);
        }

        let joined = self.hop_of(followee).await?.is_none();
        self.add_member(followee, follower_hop + 1).await?;
        Ok(joined)
    }

    async fn prepare(&self) -> ModelResult<()> {
        self.add_member(self.seed.as_ref(), 0).await?;
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
        assert!(!scope.on_follow("a", "b").await.unwrap());
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
        assert!(scope.is_in_scope("user", None).await.unwrap());
    }

    #[tokio::test]
    async fn seed_scope_reads_no_whole_feed() {
        let scope = scope_from_config(&ScopeConfig::Seed {
            seed: pk(HS_A),
            hops: 2,
        });

        assert!(scope.full_feed_homeservers(&pk(HS_A)).is_empty());
    }

    fn random_id() -> String {
        PubkyId::from(pubky::Keypair::random().public_key()).to_string()
    }

    /// Follows grow the scope one hop at a time and stop at the configured hops.
    #[tokio_shared_rt::test(shared)]
    async fn seed_scope_grows_through_follows_up_to_its_hops() -> Result<(), crate::types::DynError>
    {
        crate::StackManager::setup(&crate::StackConfig::default()).await?;

        let seed = random_id();
        let scope = SeedHopsScope::new(pk(&seed), 2);
        scope.prepare().await?;
        let (hop1, hop2, hop3, outsider) = (random_id(), random_id(), random_id(), random_id());

        assert!(scope.is_in_scope(&seed, None).await?);
        assert!(!scope.is_in_scope(&hop1, None).await?);

        assert!(
            scope.on_follow(&seed, &hop1).await?,
            "the seed's followee joins"
        );
        assert!(
            scope.on_follow(&hop1, &hop2).await?,
            "a hop-1 member's followee joins"
        );
        assert!(
            !scope.on_follow(&hop2, &hop3).await?,
            "hop 2 is the edge: its follows don't join"
        );
        assert!(
            !scope.on_follow(&outsider, &hop3).await?,
            "a non-member's follows don't join"
        );
        assert!(
            !scope.on_follow(&seed, &hop1).await?,
            "a member doesn't join twice"
        );

        assert_eq!(scope.hop_of(&hop1).await?, Some(1));
        assert_eq!(scope.hop_of(&hop2).await?, Some(2));
        assert!(!scope.is_in_scope(&hop3, None).await?);

        // A shorter path moves a member closer, so its own follows reach further.
        scope.on_follow(&seed, &hop2).await?;
        assert_eq!(scope.hop_of(&hop2).await?, Some(1));
        assert!(scope.on_follow(&hop2, &hop3).await?);
        assert_eq!(scope.hop_of(&hop3).await?, Some(2));

        // Another hop count is another scope, with its own members.
        let wider = SeedHopsScope::new(pk(&seed), 3);
        wider.prepare().await?;
        assert!(!wider.is_in_scope(&hop1, None).await?);
        Ok(())
    }
}
