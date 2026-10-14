//! Keeps who is in scope in line with the scope's setting.
//!
//! Runs once at start, before any event is read, and then periodically. A seed scope
//! also runs within seconds of a member's follow or unfollow.
//!
//! - **everything**: at start, every user kept out of a previous, narrower scope is let
//!   back in, and the HS resolver binds them.
//! - **homeservers**: users who moved off the listed homeservers leave; at start after
//!   the list changed, stubs now on a listed homeserver join.
//! - **seed**: members are worked out from the graph and compared with the last result:
//!   who is new joins, who is gone leaves.
//!
//! Joining binds a user to its homeserver with a cursor at 0; leaving unbinds it so it
//! is no longer polled. Content of users who left stays.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use deadpool_redis::redis;
use nexus_common::db::{fetch_key_from_graph, get_redis_conn, queries};
use nexus_common::models::error::ModelResult;
use nexus_common::models::scope::SeedHopsScope;
use nexus_common::models::user::stub::{is_stub, rebuild_stub_index, SCOPE_STATE_STUB};
use nexus_common::models::user::{UserCounts, UserIngestor};
use nexus_common::{ScopeConfig, WatcherConfig};
use pubky_app_specs::PubkyId;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// The scope the last start ran with, to notice a homeserver list that changed.
const ACTIVE_SCOPE_KEY: &str = "Scope:Active";
/// How often [`ScopeMaintainer::tick`] runs: the delay before a member's follow change
/// takes effect.
pub const SCOPE_MAINTENANCE_TICK_MS: u64 = 5_000;

/// How many users joined and left the scope in one run.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ScopeChange {
    pub joined: usize,
    pub left: usize,
}

/// Keeps the scope's members up to date for the watcher.
pub struct ScopeMaintainer {
    scope: ScopeConfig,
    seed_scope: Option<SeedHopsScope>,
    /// Always in scope, never leave (the moderator).
    pinned: HashSet<String>,
    ingestor: UserIngestor,
    interval: Duration,
    /// Narrows every graph-wide search to these users; `None` in production.
    /// Tests share one graph, so they pass their own users.
    limit_to: Option<Vec<String>>,
    last_run: Mutex<Instant>,
}

impl ScopeMaintainer {
    pub fn from_config(config: &WatcherConfig) -> Self {
        Self::new(config, UserIngestor::from_watcher_config(config), None)
    }

    pub fn new(
        config: &WatcherConfig,
        ingestor: UserIngestor,
        limit_to: Option<Vec<String>>,
    ) -> Self {
        let seed_scope = match &config.stack.scope {
            ScopeConfig::Seed { seed, hops } => Some(SeedHopsScope::new(seed.clone(), *hops)),
            _ => None,
        };
        Self {
            scope: config.stack.scope.clone(),
            seed_scope,
            pinned: HashSet::from([config.moderation_id.to_string()]),
            ingestor,
            interval: Duration::from_millis(config.scope_maintenance.interval_ms),
            limit_to,
            last_run: Mutex::new(Instant::now()),
        }
    }

    /// Whether the periodic task is needed: everything has nothing to maintain after start.
    pub fn is_periodic(&self) -> bool {
        self.scope != ScopeConfig::Everything
    }

    /// The run at start, before any event is read.
    pub async fn at_start(&self) -> ModelResult<ScopeChange> {
        let change = match &self.scope {
            ScopeConfig::Everything => self.let_everyone_in().await?,
            ScopeConfig::Homeservers { homeservers } => {
                let mut change = self.drop_users_bound_outside(homeservers).await?;
                if self.scope_changed().await? {
                    change.joined += self.admit_stubs_on(homeservers).await?;
                }
                change
            }
            ScopeConfig::Seed { .. } => self.recompute_members(true).await?,
        };
        self.remember_scope().await?;
        info!(?change, "Scope settled at start");
        Ok(change)
    }

    /// A periodic run: a seed scope recomputes when a member's follows changed or the
    /// interval passed; a homeserver scope checks for users who moved away each interval.
    pub async fn tick(&self) -> ModelResult<ScopeChange> {
        let mut last_run = self.last_run.lock().await;
        let interval_passed = last_run.elapsed() >= self.interval;

        let change = match (&self.scope, &self.seed_scope) {
            (ScopeConfig::Seed { .. }, Some(seed_scope)) => {
                // Taken first, so a change after this point asks for the next run.
                let stale = seed_scope.take_stale().await?;
                if !stale && !interval_passed {
                    return Ok(ScopeChange::default());
                }
                self.recompute_members(false).await?
            }
            (ScopeConfig::Homeservers { homeservers }, _) if interval_passed => {
                self.drop_users_bound_outside(homeservers).await?
            }
            _ => return Ok(ScopeChange::default()),
        };
        *last_run = Instant::now();
        if change != ScopeChange::default() {
            info!(?change, "Scope members changed");
        }
        Ok(change)
    }

    /// Works out the seed scope's members from the graph and compares them with the last
    /// result: who is new joins, who is gone leaves. Without a last result (first run,
    /// or Redis lost), `at_start` compares with every in-scope user in the graph, so a
    /// scope narrowed from a wider one lets the others go.
    async fn recompute_members(&self, at_start: bool) -> ModelResult<ScopeChange> {
        let Some(seed_scope) = &self.seed_scope else {
            return Ok(ScopeChange::default());
        };
        let members = seed_scope.members_from_graph().await?;
        let previous = match seed_scope.cached_members().await? {
            Some(previous) => previous,
            None if at_start => self.in_scope_users().await?,
            None => HashSet::new(),
        };
        // Cached first, so the per-event check already agrees while users are moved.
        seed_scope.cache_members(&members).await?;
        self.apply(&members, &previous).await
    }

    async fn apply(
        &self,
        members: &HashMap<String, u8>,
        previous: &HashSet<String>,
    ) -> ModelResult<ScopeChange> {
        let mut change = ScopeChange::default();
        for user_id in members.keys().filter(|id| !previous.contains(*id)) {
            match PubkyId::try_from(user_id.as_str()) {
                Ok(user_id) => {
                    self.ingestor.promote(&user_id).await?;
                    change.joined += 1;
                }
                Err(e) => warn!(%user_id, error = %e, "Skipping a member with an invalid id"),
            }
        }
        for user_id in previous
            .iter()
            .filter(|id| !members.contains_key(*id) && !self.pinned.contains(*id))
        {
            self.ingestor.demote(user_id).await?;
            change.left += 1;
        }
        Ok(change)
    }

    /// For "everything": clears every user's scope state. Former stubs go back into the
    /// rankings; the HS resolver binds all of them, so they are polled again.
    async fn let_everyone_in(&self) -> ModelResult<ScopeChange> {
        let query = queries::put::clear_scope_states(self.limit_to.as_deref());
        let stub_ids: Vec<String> = fetch_key_from_graph(query, "stub_ids")
            .await?
            .unwrap_or_default();
        rebuild_stub_index().await?;
        for user_id in &stub_ids {
            if let Some(counts) = UserCounts::get_from_index(user_id).await? {
                counts.put_to_index(user_id).await?;
            }
        }
        Ok(ScopeChange {
            joined: stub_ids.len(),
            left: 0,
        })
    }

    /// For a homeserver scope: users who moved to a homeserver that isn't listed leave.
    async fn drop_users_bound_outside(&self, homeservers: &[PubkyId]) -> ModelResult<ScopeChange> {
        let listed: Vec<String> = homeservers.iter().map(ToString::to_string).collect();
        let query = queries::get::get_user_ids_bound_outside(&listed, self.limit_to.as_deref());
        let outside: Vec<String> = fetch_key_from_graph(query, "user_ids")
            .await?
            .unwrap_or_default();
        let mut change = ScopeChange::default();
        for user_id in outside.iter().filter(|id| !self.pinned.contains(*id)) {
            self.ingestor.demote(user_id).await?;
            change.left += 1;
        }
        Ok(change)
    }

    /// For a homeserver list that changed: stubs whose homeserver is now listed join.
    /// Each stub's homeserver is resolved, so this runs only when the list changed.
    async fn admit_stubs_on(&self, homeservers: &[PubkyId]) -> ModelResult<usize> {
        let query = queries::get::get_user_ids_by_scope_state(SCOPE_STATE_STUB);
        let stubs: Vec<String> = fetch_key_from_graph(query, "user_ids")
            .await?
            .unwrap_or_default();
        let mut joined = 0;
        for user_id in &stubs {
            if let Some(limit_to) = &self.limit_to {
                if !limit_to.contains(user_id) {
                    continue;
                }
            }
            let Ok(user_id) = PubkyId::try_from(user_id.as_str()) else {
                continue;
            };
            let hs_id = match self.ingestor.ensure_hs_not_blacklisted(&user_id).await {
                Ok(Some(hs_id)) => hs_id,
                _ => continue,
            };
            if homeservers.iter().any(|hs| hs.as_ref() == hs_id) && is_stub(&user_id).await? {
                self.ingestor.promote(&user_id).await?;
                joined += 1;
            }
        }
        Ok(joined)
    }

    async fn in_scope_users(&self) -> ModelResult<HashSet<String>> {
        let query = queries::get::get_in_scope_user_ids(self.limit_to.as_deref());
        let users: Vec<String> = fetch_key_from_graph(query, "user_ids")
            .await?
            .unwrap_or_default();
        Ok(users.into_iter().collect())
    }

    /// Whether the scope differs from the one the last start ran with.
    async fn scope_changed(&self) -> ModelResult<bool> {
        let mut redis_conn = get_redis_conn().await?;
        let active: Option<String> = redis::cmd("GET")
            .arg(ACTIVE_SCOPE_KEY)
            .query_async(&mut redis_conn)
            .await
            .map_err(nexus_common::db::kv::RedisError::from)?;
        Ok(active != Some(self.scope_json()))
    }

    async fn remember_scope(&self) -> ModelResult<()> {
        if self.limit_to.is_some() {
            return Ok(());
        }
        let mut redis_conn = get_redis_conn().await?;
        let () = redis::cmd("SET")
            .arg(ACTIVE_SCOPE_KEY)
            .arg(self.scope_json())
            .query_async(&mut redis_conn)
            .await
            .map_err(nexus_common::db::kv::RedisError::from)?;
        Ok(())
    }

    fn scope_json(&self) -> String {
        serde_json::to_string(&self.scope).unwrap_or_default()
    }
}
