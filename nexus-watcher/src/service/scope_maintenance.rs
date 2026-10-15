//! Keeps who is in scope in line with the scope's setting.
//!
//! Runs once at start, before any event is read, and then periodically. A seed scope
//! also runs within seconds of a member's follow or unfollow.
//!
//! - **everything**: at start, every user kept out of a previous, narrower scope is let
//!   back in, and the HS resolver binds them.
//! - **homeservers**: users who moved off the listed homeservers leave; at start after
//!   the list changed, users kept out (stubs, users that left) now on a listed
//!   homeserver join.
//! - **seed**: members are worked out from the graph and compared with who is in scope:
//!   who is new joins, who is gone leaves.
//!
//! Joining binds a user to its homeserver with a cursor at 0; leaving unbinds it so it
//! is no longer polled. Content of users who left stays.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use deadpool_redis::redis::AsyncCommands;
use nexus_common::db::kv::RedisError;
use nexus_common::db::{fetch_key_from_graph, get_redis_conn, queries};
use nexus_common::models::error::{ModelError, ModelResult};
use nexus_common::models::scope::SeedHopsScope;
use nexus_common::models::user::stub::{unstub, SCOPE_STATE_STUB};
use nexus_common::models::user::{UserIngestor, SCOPE_STATE_LEFT};
use nexus_common::types::DynError;
use nexus_common::{ScopeConfig, WatcherConfig};
use pubky_app_specs::PubkyId;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// The scope the last start ran with, to notice a homeserver list that changed.
const ACTIVE_SCOPE_KEY: &str = "Scope:Active";
/// Users a homeserver scope may admit whose homeserver couldn't be found (user id ->
/// attempts): tried again each interval, up to [`MAX_ADMIT_ATTEMPTS`] times. A lookup
/// that times out looks the same as no published homeserver, so both are retried.
const ADMIT_RETRY_KEY: &str = "Scope:AdmitRetry";
const MAX_ADMIT_ATTEMPTS: u32 = 12;
/// How often [`ScopeMaintainer::tick`] runs: the delay before a member's follow change
/// takes effect.
pub const SCOPE_MAINTENANCE_TICK_MS: u64 = 5_000;

/// How many users joined and left the scope in one run.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ScopeChange {
    pub joined: usize,
    pub left: usize,
}

/// What one [`ScopeMaintainer::apply`] did.
struct Applied {
    change: ScopeChange,
    /// Users that left the scope.
    left: Vec<String>,
    /// New members that couldn't be brought in.
    not_admitted: Vec<String>,
}

/// What there is to maintain for each kind of scope.
enum Kind {
    Everything,
    Homeservers(Vec<PubkyId>),
    Seed(Box<SeedHopsScope>),
}

/// Keeps the scope's members up to date for the watcher.
pub struct ScopeMaintainer {
    kind: Kind,
    /// The scope as configured, to notice a change between starts.
    scope_json: String,
    /// Always in scope and never let go, whatever the scope.
    moderator: PubkyId,
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
        let kind = match &config.stack.scope {
            ScopeConfig::Everything => Kind::Everything,
            ScopeConfig::Homeservers { homeservers } => Kind::Homeservers(homeservers.clone()),
            ScopeConfig::Seed { seed, hops } => {
                Kind::Seed(Box::new(SeedHopsScope::new(seed.clone(), *hops)))
            }
        };
        Self {
            kind,
            scope_json: serde_json::to_string(&config.stack.scope).unwrap_or_default(),
            moderator: config.moderation_id.clone(),
            ingestor,
            interval: Duration::from_millis(config.scope_maintenance.interval_ms),
            limit_to,
            last_run: Mutex::new(Instant::now()),
        }
    }

    /// The seed scope this maintainer keeps, if it is one.
    pub fn seed_scope(&self) -> Option<&SeedHopsScope> {
        match &self.kind {
            Kind::Seed(seed_scope) => Some(seed_scope),
            _ => None,
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
            Kind::Seed(seed_scope) => self.recompute_members(seed_scope, true).await?,
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

        let change = match &self.kind {
            Kind::Seed(seed_scope) => {
                // Taken first, so a change after this point asks for the next run.
                let stale = seed_scope.take_stale().await?;
                if !stale && !interval_passed {
                    return Ok(ScopeChange::default());
                }
                self.recompute_members(seed_scope, false).await?
            }
            Kind::Homeservers(homeservers) if interval_passed => {
                let mut change = self.drop_users_bound_outside(homeservers).await?;
                change.joined += self.retry_admissions(homeservers).await?;
                change
            }
            _ => return Ok(ScopeChange::default()),
        };
        *last_run = Instant::now();
        if change != ScopeChange::default() {
            info!(?change, "Scope members changed");
        }
        Ok(change)
    }

    /// Recomputes a seed scope's members now, e.g. right after a big Nexus suggested some.
    pub async fn recompute_now(&self) -> ModelResult<ScopeChange> {
        let Some(seed_scope) = self.seed_scope() else {
            return Ok(ScopeChange::default());
        };
        let mut last_run = self.last_run.lock().await;
        let change = self.recompute_members(seed_scope, false).await?;
        *last_run = Instant::now();
        Ok(change)
    }

    /// Works out the seed scope's members and compares them with who is in scope now:
    /// who is new joins, who is gone leaves.
    ///
    /// At start, and whenever there is no cached result, "in scope now" comes from the
    /// graph, which records what was actually applied. The cache can be from another
    /// scope (seed, then everything, then the same seed again) or hold changes a crash
    /// interrupted. Between starts, the cache is used and kept true: a user only leaves it
    /// once it has left, and joins it only once it has joined.
    async fn recompute_members(
        &self,
        seed_scope: &SeedHopsScope,
        at_start: bool,
    ) -> ModelResult<ScopeChange> {
        let members = seed_scope.members().await?;
        let previous = match seed_scope.cached_members().await? {
            Some(cached) if !at_start => cached,
            _ => self.in_scope_users().await?,
        };
        // Cached first, so the per-event check already agrees while users are moved. Users
        // on their way out stay cached until they have left: they are still polled.
        let mut cached = members.clone();
        for user_id in previous.iter().filter(|id| !members.contains_key(*id)) {
            cached.insert(user_id.clone(), seed_scope.hops());
        }
        seed_scope.cache_members(&cached).await?;
        let applied = self.apply(&members, &previous).await;
        // Out of the cache: the users that left, and the users that couldn't join, so the
        // next recompute sees them as new and retries. Not forced sooner: a user that
        // keeps failing (no published homeserver) is tried again at the next follow
        // change or interval, not every tick. A user that couldn't leave stays cached, so
        // the next recompute tries again.
        let settled: Vec<String> = applied
            .left
            .into_iter()
            .chain(applied.not_admitted)
            .collect();
        seed_scope.forget_members(&settled).await?;
        Ok(applied.change)
    }

    /// Brings in who is new and lets go who is gone. One user failing (a lookup timing
    /// out, no published homeserver) is logged and doesn't stop the others; the caller
    /// is told who left and who couldn't join, and keeps the rest to try again.
    async fn apply(&self, members: &HashMap<String, u8>, previous: &HashSet<String>) -> Applied {
        let mut change = ScopeChange::default();
        let mut left = Vec::new();
        let mut not_admitted = Vec::new();
        for user_id in members.keys().filter(|id| !previous.contains(*id)) {
            match self.admit(user_id).await {
                Ok(true) => change.joined += 1,
                Ok(false) => {
                    warn!(%user_id, "Could not bring a member in: no published homeserver");
                    not_admitted.push(user_id.clone());
                }
                Err(e) => {
                    warn!(%user_id, error = %e, "Could not bring a member in");
                    not_admitted.push(user_id.clone());
                }
            }
        }
        let gone = previous
            .iter()
            .filter(|id| !members.contains_key(*id) && **id != self.moderator.as_ref());
        for user_id in gone {
            match self.ingestor.demote(user_id).await {
                Ok(()) => {
                    change.left += 1;
                    left.push(user_id.clone());
                }
                Err(e) => warn!(%user_id, error = %e, "Could not let a member go"),
            }
        }
        Applied {
            change,
            left,
            not_admitted,
        }
    }

    /// Brings a member in: ingested if new (in scope now, so bound and polled), promoted
    /// if it was kept out. Returns whether it is in scope now.
    async fn admit(&self, user_id: &str) -> Result<bool, DynError> {
        let user_id = PubkyId::try_from(user_id)?;
        self.ingestor.maybe_ingest_user(&user_id).await?;
        Ok(self.ingestor.promote(&user_id).await?)
    }

    /// For "everything": clears every user's scope state, and former stubs go back into
    /// the rankings; the HS resolver binds them all, so they are polled again.
    ///
    /// Stubs are brought into the rankings and feeds before the graph's states are
    /// cleared, so a start that fails partway finds them still marked and tries again.
    async fn let_everyone_in(&self) -> ModelResult<ScopeChange> {
        let query = queries::get::get_user_ids_by_scope_state(SCOPE_STATE_STUB);
        let stub_ids: Vec<String> = fetch_key_from_graph(query, "user_ids")
            .await?
            .unwrap_or_default();
        let stub_ids: Vec<String> = stub_ids
            .into_iter()
            .filter(|id| {
                self.limit_to
                    .as_ref()
                    .is_none_or(|limit| limit.contains(id))
            })
            .collect();
        for user_id in &stub_ids {
            unstub(user_id).await?;
        }
        let query = queries::put::clear_scope_states(self.limit_to.as_deref());
        let _: Option<Vec<String>> = fetch_key_from_graph(query, "stub_ids").await?;
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
        for user_id in outside.iter().filter(|id| **id != self.moderator.as_ref()) {
            self.ingestor.demote(user_id).await?;
            change.left += 1;
        }
        Ok(change)
    }

    /// For a homeserver list that changed: users kept out (stubs, and users that left
    /// when the list was narrowed) whose homeserver is now listed join. Each one's
    /// homeserver is resolved, so this runs only when the list changed.
    async fn admit_outsiders_on(&self, homeservers: &[PubkyId]) -> ModelResult<usize> {
        let mut outsiders: Vec<String> = Vec::new();
        for scope_state in [SCOPE_STATE_STUB, SCOPE_STATE_LEFT] {
            let query = queries::get::get_user_ids_by_scope_state(scope_state);
            let user_ids: Option<Vec<String>> = fetch_key_from_graph(query, "user_ids").await?;
            outsiders.extend(user_ids.unwrap_or_default());
        }
        self.admit_if_listed(homeservers, &self.limited(outsiders))
            .await
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
