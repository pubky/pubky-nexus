//! Keeps who is in scope in line with the scope's setting.
//!
//! Runs once at start, before any event is read, and then periodically.
//!
//! - **everything**: at start, every user kept out of a previous, narrower scope is let
//!   back in.
//! - **homeservers**: users bound to a homeserver the list doesn't hold leave; at start
//!   after the list changed, users kept out (outsiders, users that left) now on a listed
//!   homeserver join.
//!
//! Joining binds a user to its homeserver with a cursor at 0; leaving unbinds it so it
//! is no longer polled. Content of users who left stays.

use deadpool_redis::redis::AsyncCommands;
use nexus_common::db::kv::RedisError;
use nexus_common::db::{fetch_key_from_graph, get_redis_conn, queries};
use nexus_common::models::error::{ModelError, ModelResult};
use nexus_common::models::user::{
    PkdnsHomeserverResolver, UserIngestor, SCOPE_STATE_LEFT, SCOPE_STATE_OUTSIDE,
};
use nexus_common::{ScopeConfig, WatcherConfig};

use crate::scope::watcher_ingestor;
use pubky_app_specs::PubkyId;
use std::sync::Arc;
use tracing::{info, warn};

/// The scope the last start ran with, to notice a homeserver list that changed.
const ACTIVE_SCOPE_KEY: &str = "Scope:Active";
/// Users a homeserver scope may admit whose homeserver couldn't be found (user id ->
/// attempts): tried again each interval, up to [`MAX_ADMIT_ATTEMPTS`] times. A lookup
/// that times out looks the same as no published homeserver, so both are retried. Past
/// that, or if Redis loses the list, the user is admitted at the next list change.
const ADMIT_RETRY_KEY: &str = "Scope:AdmitRetry";
const MAX_ADMIT_ATTEMPTS: u32 = 12;

/// How many users joined and left the scope in one run.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ScopeChange {
    pub joined: usize,
    pub left: usize,
}

/// What there is to maintain for each kind of scope.
enum Kind {
    Everything,
    Homeservers(Vec<PubkyId>),
}

/// Keeps the scope's members up to date for the watcher.
pub struct ScopeMaintainer {
    kind: Kind,
    /// The scope as configured, to notice a change between starts.
    scope_json: String,
    /// Always in scope and never let go, whatever the scope.
    moderator: PubkyId,
    ingestor: UserIngestor,
    /// Narrows every graph-wide search to these users; `None` in production.
    /// Tests share one graph, so they pass their own users.
    limit_to: Option<Vec<String>>,
}

impl ScopeMaintainer {
    /// Looks users' homeservers up through `resolver`.
    pub fn from_config(config: &WatcherConfig, resolver: Arc<dyn PkdnsHomeserverResolver>) -> Self {
        Self::new(config, watcher_ingestor(config, resolver), None)
    }

    pub fn new(
        config: &WatcherConfig,
        ingestor: UserIngestor,
        limit_to: Option<Vec<String>>,
    ) -> Self {
        let kind = match &config.stack.scope {
            ScopeConfig::Everything => Kind::Everything,
            ScopeConfig::Homeservers { homeservers } => Kind::Homeservers(homeservers.clone()),
        };
        Self {
            kind,
            scope_json: serde_json::to_string(&config.stack.scope).unwrap_or_default(),
            moderator: config.moderation_id.clone(),
            ingestor,
            limit_to,
        }
    }

    /// Whether the periodic task is needed: everything has nothing to maintain after start.
    pub fn is_periodic(&self) -> bool {
        !matches!(self.kind, Kind::Everything)
    }

    /// The run at start, before any event is read.
    pub async fn at_start(&self) -> ModelResult<ScopeChange> {
        let change = match &self.kind {
            Kind::Everything => self.let_everyone_in().await?,
            Kind::Homeservers(homeservers) => {
                let mut change = self.drop_users_bound_outside(homeservers).await?;
                if self.scope_changed().await? {
                    change.joined += self.admit_outsiders_on(homeservers).await?;
                }
                change
            }
        };
        self.remember_scope().await?;
        info!(?change, "Scope settled at start");
        Ok(change)
    }

    /// A periodic run: a homeserver scope lets go of users bound outside its list, and
    /// tries again the admissions whose homeserver couldn't be found.
    pub async fn tick(&self) -> ModelResult<ScopeChange> {
        let Kind::Homeservers(homeservers) = &self.kind else {
            return Ok(ScopeChange::default());
        };
        let mut change = self.drop_users_bound_outside(homeservers).await?;
        change.joined += self.retry_admissions(homeservers).await?;
        if change != ScopeChange::default() {
            info!(?change, "Scope members changed");
        }
        Ok(change)
    }

    /// For "everything": lets every outsider and every user that left back in. Each one is
    /// bound with its cursor at 0 and catches up, so the records it wrote while it was out
    /// are read, also on the primary homeserver, whose feed moved past them. One that
    /// fails stays out, and the next start tries it again.
    async fn let_everyone_in(&self) -> ModelResult<ScopeChange> {
        let outsiders = self.limited(self.outsiders().await?);
        let mut change = ScopeChange::default();
        for user_id in &outsiders {
            let Ok(id) = PubkyId::try_from(user_id.as_str()) else {
                continue;
            };
            match self.ingestor.let_in(&id).await {
                Ok(()) => change.joined += 1,
                Err(e) => {
                    warn!(%user_id, error = %e, "Could not let a user in; trying again next start")
                }
            }
        }
        Ok(change)
    }

    /// Users kept out of the scope: outsiders, and users that left.
    async fn outsiders(&self) -> ModelResult<Vec<String>> {
        let mut outsiders = Vec::new();
        for scope_state in [SCOPE_STATE_OUTSIDE, SCOPE_STATE_LEFT] {
            let query = queries::get::get_user_ids_by_scope_state(scope_state);
            let user_ids: Option<Vec<String>> = fetch_key_from_graph(query, "user_ids").await?;
            outsiders.extend(user_ids.unwrap_or_default());
        }
        Ok(outsiders)
    }

    /// For a homeserver scope: users bound to a homeserver that isn't listed leave.
    async fn drop_users_bound_outside(&self, homeservers: &[PubkyId]) -> ModelResult<ScopeChange> {
        let listed: Vec<String> = homeservers.iter().map(ToString::to_string).collect();
        let query = queries::get::get_user_ids_bound_outside(&listed, self.limit_to.as_deref());
        let outside: Vec<String> = fetch_key_from_graph(query, "user_ids")
            .await?
            .unwrap_or_default();
        let mut change = ScopeChange::default();
        for user_id in outside.iter().filter(|id| **id != self.moderator.as_ref()) {
            self.ingestor.demote(user_id).await?;
            change.left += 1;
        }
        Ok(change)
    }

    /// For a homeserver list that changed: users kept out (outsiders, and users that left
    /// when the list was narrowed) whose homeserver is now listed join. Each one's
    /// homeserver is resolved, so this runs only when the list changed.
    async fn admit_outsiders_on(&self, homeservers: &[PubkyId]) -> ModelResult<usize> {
        let outsiders = self.limited(self.outsiders().await?);
        self.admit_if_listed(homeservers, &outsiders).await
    }

    /// Tries again the admissions whose homeserver lookup failed.
    async fn retry_admissions(&self, homeservers: &[PubkyId]) -> ModelResult<usize> {
        let mut redis_conn = get_redis_conn().await?;
        let waiting: Vec<String> = redis_conn
            .hkeys(ADMIT_RETRY_KEY)
            .await
            .map_err(RedisError::from)?;
        self.admit_if_listed(homeservers, &self.limited(waiting))
            .await
    }

    /// Admits each user whose homeserver is listed, and returns how many joined. A user
    /// whose homeserver couldn't be found is kept in [`ADMIT_RETRY_KEY`] to try again; one
    /// on a blacklisted homeserver is not eligible.
    async fn admit_if_listed(
        &self,
        homeservers: &[PubkyId],
        user_ids: &[String],
    ) -> ModelResult<usize> {
        let mut redis_conn = get_redis_conn().await?;
        let mut joined = 0;
        for user_id in user_ids {
            let Ok(id) = PubkyId::try_from(user_id.as_str()) else {
                continue;
            };
            // `None`: on a blacklisted homeserver, so never admitted.
            let hs_id = match self.ingestor.ensure_hs_not_blacklisted(&id).await {
                Ok(Some(hs_id)) => Some(hs_id),
                Err(ModelError::HsBlacklisted { .. }) => None,
                Ok(None) | Err(_) => {
                    self.retry_admission_later(&mut redis_conn, user_id).await?;
                    continue;
                }
            };
            let listed =
                hs_id.is_some_and(|hs_id| homeservers.iter().any(|hs| hs.as_ref() == hs_id));
            if listed {
                // One user failing doesn't stop the others: it is tried again.
                match self.ingestor.promote(&id).await {
                    Ok(true) => joined += 1,
                    result => {
                        let error = result.err().map(|e| e.to_string());
                        warn!(%user_id, ?error, "Could not bring a user in; trying again next interval");
                        self.retry_admission_later(&mut redis_conn, user_id).await?;
                        continue;
                    }
                }
            }
            let () = redis_conn
                .hdel(ADMIT_RETRY_KEY, user_id)
                .await
                .map_err(RedisError::from)?;
        }
        Ok(joined)
    }

    /// Counts one more attempt to admit the user, and gives up past
    /// [`MAX_ADMIT_ATTEMPTS`].
    async fn retry_admission_later(
        &self,
        redis_conn: &mut deadpool_redis::Connection,
        user_id: &str,
    ) -> ModelResult<()> {
        let attempts: u32 = redis_conn
            .hincr(ADMIT_RETRY_KEY, user_id, 1)
            .await
            .map_err(RedisError::from)?;
        if attempts > MAX_ADMIT_ATTEMPTS {
            warn!(%user_id, "Could not bring a user the scope may admit in; giving up");
            let () = redis_conn
                .hdel(ADMIT_RETRY_KEY, user_id)
                .await
                .map_err(RedisError::from)?;
        }
        Ok(())
    }

    /// `user_ids` narrowed to [`Self::limit_to`], if set.
    fn limited(&self, user_ids: Vec<String>) -> Vec<String> {
        match &self.limit_to {
            Some(limit) => user_ids
                .into_iter()
                .filter(|id| limit.contains(id))
                .collect(),
            None => user_ids,
        }
    }

    /// Whether the scope differs from the one the last start ran with.
    async fn scope_changed(&self) -> ModelResult<bool> {
        let mut redis_conn = get_redis_conn().await?;
        let active: Option<String> = redis_conn
            .get(ACTIVE_SCOPE_KEY)
            .await
            .map_err(RedisError::from)?;
        Ok(active.as_deref() != Some(self.scope_json.as_str()))
    }

    /// Records the scope this start ran with. Tests (`limit_to`) leave it alone, as they
    /// don't run the whole graph's scope.
    async fn remember_scope(&self) -> ModelResult<()> {
        if self.limit_to.is_some() {
            return Ok(());
        }
        let mut redis_conn = get_redis_conn().await?;
        let () = redis_conn
            .set(ACTIVE_SCOPE_KEY, &self.scope_json)
            .await
            .map_err(RedisError::from)?;
        Ok(())
    }
}
