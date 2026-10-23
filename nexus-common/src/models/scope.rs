//! Which users this Nexus indexes.
//!
//! The watcher asks one [`Scope`], built from [`ScopeConfig`], at every point where
//! it decides whose content to read: which homeservers it reads as a whole feed,
//! whether a newly discovered user is indexed or kept as an outsider, and whether an
//! event's author is indexed.

use std::collections::HashSet;
use std::sync::Arc;

use pubky_app_specs::PubkyId;

use crate::config::{ScopeConfig, WatcherConfig};
use crate::db::{fetch_row_from_graph, queries};
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
}

/// Builds the scope the configuration describes.
pub fn scope_from_config(config: &ScopeConfig) -> Arc<dyn Scope> {
    match config {
        ScopeConfig::Everything => Arc::new(EverythingScope),
        ScopeConfig::Homeservers { homeservers } => Arc::new(HomeserverListScope::new(homeservers)),
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
            ScopeConfig::Everything => &[],
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
}
