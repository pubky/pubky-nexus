//! Which users this Nexus indexes.
//!
//! The watcher and the API ask one [`Scope`], built from [`ScopeConfig`], whether a
//! user's content is indexed: the user ingestor keeps one, so both decide a newly met
//! user is a member or an outsider the same way, and the watcher skips events of
//! authors outside it. What only the watcher needs (the moderator pinned into it, and
//! which homeservers it reads as a whole feed) lives in `nexus-watcher`.

use std::sync::Arc;

use pubky_app_specs::PubkyId;

use crate::config::ScopeConfig;
use crate::db::{fetch_key_from_graph, fetch_row_from_graph, queries};
use crate::models::error::ModelResult;

/// The answers every scope gives.
#[async_trait::async_trait]
pub trait Scope: Send + Sync + std::fmt::Debug {
    /// The settings this scope was built from, as `/v0/info` publishes them.
    fn config(&self) -> &ScopeConfig;

    /// Whether the user's own content is indexed. `hs_id` is the user's homeserver when
    /// the caller knows it; otherwise the scope looks it up if it needs one.
    async fn is_in_scope(&self, user_id: &str, hs_id: Option<&str>) -> ModelResult<bool>;
}

/// Builds the scope the configuration describes.
pub fn scope_from_config(config: &ScopeConfig) -> Arc<dyn Scope> {
    match config {
        ScopeConfig::Everything => Arc::new(EverythingScope),
        ScopeConfig::Homeservers { homeservers } => Arc::new(HomeserverListScope::new(homeservers)),
        ScopeConfig::Seed { seed, hops } => Arc::new(SeedScope::new(seed.clone(), *hops)),
    }
}

/// Every user Nexus can discover, as before scopes existed.
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
}

/// Users hosted on a fixed list of homeservers.
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
            ScopeConfig::Everything | ScopeConfig::Seed { .. } => &[],
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
}

/// The seed user and everyone it reaches by following, up to `hops` follows away.
///
/// Who that is changes with every follow, so the watcher works it out from the graph and
/// applies it there (see its scope maintenance): a member has no `scope_state`, and
/// everyone else it knows is an outsider or a member that left. The graph answers, so the
/// watcher and the API agree without a copy to keep in step. A user never met is out until
/// a recompute finds it among the members.
#[derive(Debug)]
pub struct SeedScope {
    config: ScopeConfig,
}

impl SeedScope {
    pub fn new(seed: PubkyId, hops: u8) -> Self {
        Self {
            config: ScopeConfig::Seed { seed, hops },
        }
    }

    fn seed(&self) -> &str {
        match &self.config {
            ScopeConfig::Seed { seed, .. } => seed,
            ScopeConfig::Everything | ScopeConfig::Homeservers { .. } => "",
        }
    }
}

#[async_trait::async_trait]
impl Scope for SeedScope {
    fn config(&self) -> &ScopeConfig {
        &self.config
    }

    /// The seed is always in; anyone else is in once the graph holds it without a
    /// `scope_state`.
    async fn is_in_scope(&self, user_id: &str, _hs_id: Option<&str>) -> ModelResult<bool> {
        if user_id == self.seed() {
            return Ok(true);
        }
        let query = queries::get::get_user_scope_state(user_id);
        let scope_state: Option<Option<String>> =
            fetch_key_from_graph(query, "scope_state").await?;
        Ok(matches!(scope_state, Some(None)))
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
    async fn everything_indexes_everyone() {
        let scope = scope_from_config(&ScopeConfig::Everything);

        assert!(scope.is_in_scope("anyone", None).await.unwrap());
        assert!(scope.is_in_scope("anyone", Some(HS_B)).await.unwrap());
    }

    #[tokio::test]
    async fn homeserver_list_indexes_its_homeservers_users() {
        let scope = scope_from_config(&ScopeConfig::Homeservers {
            homeservers: vec![pk(HS_B)],
        });

        assert!(scope.is_in_scope("user", Some(HS_B)).await.unwrap());
        assert!(!scope.is_in_scope("user", Some(HS_A)).await.unwrap());
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
