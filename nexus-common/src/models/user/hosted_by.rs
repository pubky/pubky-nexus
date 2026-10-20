//! A user's `HOSTED_BY` binding and how a resolved `_pubky` record is applied
//! to it. Shared by the watcher's periodic resolver and the API's recheck.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::{exec_single_row, fetch_row_from_graph, queries, GraphResult};
use pubky_app_specs::PubkyId;

/// What happens to a bound user whose record no longer names the stored
/// homeserver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StaleMapping {
    /// Keep the mapping and mark it `stale`: the user is indexed nowhere until
    /// the record names the stored homeserver again. The default.
    #[default]
    Enabled,
    /// Follow the record: a record naming another homeserver moves the mapping
    /// there, a record naming none keeps it. A stale flag left from before is
    /// cleared either way.
    Disabled,
}

/// Why a mapping was marked stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaleReason {
    /// PKDNS returned no homeserver.
    Unresolved,
    /// PKDNS returned another homeserver.
    HsChanged,
}

/// A user's stored mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeserverMapping {
    pub hs_id: String,
    pub stale: bool,
}

/// What applying a resolved record did to the mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingUpdate {
    /// No mapping and nothing published; graph untouched.
    Unbound,
    /// No mapping before; bound to the published homeserver now.
    Bound,
    /// The published homeserver is the stored one; the stale flag is cleared.
    Confirmed { was_stale: bool },
    /// The published homeserver is missing or another one; the mapping is kept
    /// and marked stale ([`StaleMapping::Enabled`]).
    MarkedStale {
        reason: StaleReason,
        was_stale: bool,
    },
    /// The mapping moved to the published homeserver ([`StaleMapping::Disabled`]).
    Switched { from: String, was_stale: bool },
    /// Nothing published; the mapping is kept and the stale flag cleared
    /// ([`StaleMapping::Disabled`]).
    Kept { was_stale: bool },
}

/// Returns the user's stored mapping, if any.
pub async fn get_user_homeserver(user_id: &str) -> GraphResult<Option<HomeserverMapping>> {
    let query = queries::get::get_user_homeserver(user_id);
    let Some(row) = fetch_row_from_graph(query).await? else {
        return Ok(None);
    };
    Ok(Some(HomeserverMapping {
        hs_id: row.get("homeserver_id")?,
        stale: row.get("stale")?,
    }))
}

/// Binds a user to their homeserver, recording the `HOSTED_BY` relationship and `resolved_at`.
pub async fn set_user_homeserver(user_id: &str, homeserver_id: &str) -> GraphResult<()> {
    exec_single_row(queries::put::set_user_homeserver(user_id, homeserver_id)).await
}

/// Toggles the stale flag on a user's existing homeserver mapping.
pub async fn set_user_homeserver_stale(user_id: &str, stale: bool) -> GraphResult<()> {
    exec_single_row(queries::put::set_user_homeserver_stale(user_id, stale)).await
}

/// Applies what PKDNS says about `user_id`'s homeserver to the stored mapping.
///
/// `resolved` is the homeserver the record names, or `None` when there is no
/// record or it names none. The user node must exist.
pub async fn apply_resolved_homeserver(
    user_id: &str,
    resolved: Option<&PubkyId>,
    stale_mapping: StaleMapping,
) -> GraphResult<MappingUpdate> {
    let stored = get_user_homeserver(user_id).await?;

    let update = match (stored, resolved) {
        (None, None) => MappingUpdate::Unbound,

        (None, Some(hs_id)) => {
            set_user_homeserver(user_id, hs_id).await?;
            MappingUpdate::Bound
        }

        (Some(stored), Some(hs_id)) if hs_id.as_ref() == stored.hs_id => {
            set_user_homeserver_stale(user_id, false).await?;
            MappingUpdate::Confirmed {
                was_stale: stored.stale,
            }
        }

        (Some(stored), resolved) => match stale_mapping {
            StaleMapping::Enabled => {
                set_user_homeserver_stale(user_id, true).await?;
                MappingUpdate::MarkedStale {
                    reason: match resolved {
                        None => StaleReason::Unresolved,
                        Some(_) => StaleReason::HsChanged,
                    },
                    was_stale: stored.stale,
                }
            }
            StaleMapping::Disabled => match resolved {
                Some(hs_id) => {
                    set_user_homeserver(user_id, hs_id).await?;
                    MappingUpdate::Switched {
                        from: stored.hs_id,
                        was_stale: stored.stale,
                    }
                }
                None => {
                    set_user_homeserver_stale(user_id, false).await?;
                    MappingUpdate::Kept {
                        was_stale: stored.stale,
                    }
                }
            },
        },
    };

    Ok(update)
}

/// The result of re-checking a user's homeserver against the network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct HomeserverRecheck {
    /// The homeserver the user is bound to after the check, if any.
    pub homeserver: Option<String>,
    pub outcome: RecheckOutcome,
}

/// What a recheck did to the user's mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecheckOutcome {
    /// Nothing is published and the user has no mapping.
    Unbound,
    /// The user had no mapping and is now bound to the published homeserver.
    Bound,
    /// The published homeserver is the one the user was bound to.
    Confirmed,
    /// The mapping moved to the published homeserver.
    Switched,
    /// Nothing is published; the mapping was kept.
    Kept,
}

impl HomeserverRecheck {
    /// Pairs a [`StaleMapping::Disabled`] update with the resulting binding.
    /// A `MarkedStale` update cannot come from a recheck; it reads as kept.
    pub fn new(update: &MappingUpdate, homeserver: Option<String>) -> Self {
        let outcome = match update {
            MappingUpdate::Unbound => RecheckOutcome::Unbound,
            MappingUpdate::Bound => RecheckOutcome::Bound,
            MappingUpdate::Confirmed { .. } => RecheckOutcome::Confirmed,
            MappingUpdate::Switched { .. } => RecheckOutcome::Switched,
            MappingUpdate::Kept { .. } | MappingUpdate::MarkedStale { .. } => RecheckOutcome::Kept,
        };
        Self {
            homeserver,
            outcome,
        }
    }
}
