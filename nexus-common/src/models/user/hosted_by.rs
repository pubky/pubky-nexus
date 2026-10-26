//! A user's `HOSTED_BY` binding and how a resolved `_pubky` record is applied
//! to it. Shared by the watcher's periodic resolver and the API's recheck.

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
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

/// What a resolved record does to the stored mapping: decided by
/// [`Self::decide`], written by [`Self::apply`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingUpdate {
    /// No mapping and nothing published; graph untouched.
    Unbound,
    /// No mapping before; bound to the published homeserver now.
    Bound { to: String },
    /// The published homeserver is the stored one; the stale flag is cleared.
    Confirmed { was_stale: bool },
    /// The published homeserver is missing or another one; the mapping is kept
    /// and marked stale ([`StaleMapping::Enabled`]).
    MarkedStale {
        reason: StaleReason,
        was_stale: bool,
    },
    /// The mapping moved to the published homeserver ([`StaleMapping::Disabled`]).
    Switched {
        from: String,
        to: String,
        was_stale: bool,
    },
    /// Nothing published; the mapping is kept and the stale flag cleared
    /// ([`StaleMapping::Disabled`]).
    Kept { was_stale: bool },
}

impl MappingUpdate {
    /// What PKDNS's answer does to the stored mapping. `resolved` is the
    /// homeserver the record names, or `None` when there is no record or it
    /// names none.
    pub fn decide(
        stored: Option<&HomeserverMapping>,
        resolved: Option<&PubkyId>,
        stale_mapping: StaleMapping,
    ) -> Self {
        let Some(stored) = stored else {
            return match resolved {
                None => Self::Unbound,
                Some(hs_id) => Self::Bound {
                    to: hs_id.to_string(),
                },
            };
        };
        let was_stale = stored.stale;
        match (resolved, stale_mapping) {
            (Some(hs_id), _) if hs_id.as_ref() == stored.hs_id => Self::Confirmed { was_stale },
            (resolved, StaleMapping::Enabled) => Self::MarkedStale {
                reason: match resolved {
                    None => StaleReason::Unresolved,
                    Some(_) => StaleReason::HsChanged,
                },
                was_stale,
            },
            (Some(hs_id), StaleMapping::Disabled) => Self::Switched {
                from: stored.hs_id.clone(),
                to: hs_id.to_string(),
                was_stale,
            },
            (None, StaleMapping::Disabled) => Self::Kept { was_stale },
        }
    }

    /// Writes the update to `user_id`'s mapping. The user node must exist.
    /// Every variant but [`Self::Unbound`] refreshes `resolved_at`.
    pub async fn apply(&self, user_id: &str) -> GraphResult<()> {
        match self {
            Self::Unbound => Ok(()),
            Self::Bound { to } => {
                debug!(%user_id, homeserver = %to, "HS mapping created");
                set_user_homeserver(user_id, to).await
            }
            Self::Confirmed { .. } => {
                debug!(%user_id, "HS mapping still active");
                set_user_homeserver_stale(user_id, false).await
            }
            Self::MarkedStale { reason, .. } => {
                warn!(%user_id, ?reason, "User homeserver changed or was removed; switching disabled, mapping marked stale");
                set_user_homeserver_stale(user_id, true).await
            }
            Self::Switched { from, to, .. } => {
                info!(%user_id, %from, %to, "User published another homeserver; HS mapping switched");
                set_user_homeserver(user_id, to).await
            }
            Self::Kept { .. } => {
                debug!(%user_id, "PKDNS returned no homeserver; HS mapping kept");
                set_user_homeserver_stale(user_id, false).await
            }
        }
    }

    /// The homeserver the user is bound to once the update is applied to
    /// `stored`, if any.
    pub fn homeserver_after(&self, stored: Option<&HomeserverMapping>) -> Option<String> {
        match self {
            Self::Bound { to } | Self::Switched { to, .. } => Some(to.clone()),
            Self::Unbound => None,
            Self::Confirmed { .. } | Self::MarkedStale { .. } | Self::Kept { .. } => {
                stored.map(|mapping| mapping.hs_id.clone())
            }
        }
    }

    /// The homeserver the update binds the user to, when it binds one.
    pub fn bound_to(&self) -> Option<&str> {
        match self {
            Self::Bound { to } | Self::Switched { to, .. } => Some(to),
            Self::Unbound
            | Self::Confirmed { .. }
            | Self::MarkedStale { .. }
            | Self::Kept { .. } => None,
        }
    }
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

/// A recheck follows the record ([`StaleMapping::Disabled`]), so it never
/// marks a mapping stale; should one arrive anyway, it reads as kept.
impl From<&MappingUpdate> for RecheckOutcome {
    fn from(update: &MappingUpdate) -> Self {
        match update {
            MappingUpdate::Unbound => Self::Unbound,
            MappingUpdate::Bound { .. } => Self::Bound,
            MappingUpdate::Confirmed { .. } => Self::Confirmed,
            MappingUpdate::Switched { .. } => Self::Switched,
            MappingUpdate::Kept { .. } | MappingUpdate::MarkedStale { .. } => Self::Kept,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::random_pubky_id;

    /// Every combination of stored mapping, resolved record and policy maps to
    /// one update, and the policy only matters when the record and the stored
    /// mapping disagree.
    #[test]
    fn decide_covers_every_case() {
        use MappingUpdate::*;
        use StaleMapping::{Disabled, Enabled};
        use StaleReason::{HsChanged, Unresolved};

        let (a, b) = (random_pubky_id(), random_pubky_id());
        let on_a = |stale| HomeserverMapping {
            hs_id: a.to_string(),
            stale,
        };
        let switched = |was_stale| Switched {
            from: a.to_string(),
            to: b.to_string(),
            was_stale,
        };
        let cases = [
            (None, None, Enabled, Unbound),
            (None, None, Disabled, Unbound),
            (None, Some(&a), Enabled, Bound { to: a.to_string() }),
            (None, Some(&a), Disabled, Bound { to: a.to_string() }),
            (
                Some(on_a(false)),
                Some(&a),
                Enabled,
                Confirmed { was_stale: false },
            ),
            (
                Some(on_a(true)),
                Some(&a),
                Disabled,
                Confirmed { was_stale: true },
            ),
            (
                Some(on_a(false)),
                Some(&b),
                Enabled,
                MarkedStale {
                    reason: HsChanged,
                    was_stale: false,
                },
            ),
            (
                Some(on_a(true)),
                None,
                Enabled,
                MarkedStale {
                    reason: Unresolved,
                    was_stale: true,
                },
            ),
            (Some(on_a(false)), Some(&b), Disabled, switched(false)),
            (Some(on_a(true)), Some(&b), Disabled, switched(true)),
            (Some(on_a(false)), None, Disabled, Kept { was_stale: false }),
            (Some(on_a(true)), None, Disabled, Kept { was_stale: true }),
        ];
        for (stored, resolved, policy, expected) in cases {
            assert_eq!(
                MappingUpdate::decide(stored.as_ref(), resolved, policy),
                expected,
                "stored {stored:?}, resolved {resolved:?}, {policy:?}"
            );
        }
    }

    /// After an update the user is bound to the homeserver it bound, or to
    /// the stored one when it kept it, or to none.
    #[test]
    fn homeserver_after_follows_the_update() {
        use MappingUpdate::*;

        let (a, b) = (random_pubky_id(), random_pubky_id());
        let on_a = Some(HomeserverMapping {
            hs_id: a.to_string(),
            stale: false,
        });
        let cases = [
            (Unbound, None, None),
            (Bound { to: b.to_string() }, None, Some(b.to_string())),
            (
                Confirmed { was_stale: false },
                on_a.clone(),
                Some(a.to_string()),
            ),
            (
                MarkedStale {
                    reason: StaleReason::Unresolved,
                    was_stale: true,
                },
                on_a.clone(),
                Some(a.to_string()),
            ),
            (
                Switched {
                    from: a.to_string(),
                    to: b.to_string(),
                    was_stale: false,
                },
                on_a.clone(),
                Some(b.to_string()),
            ),
            (Kept { was_stale: true }, on_a.clone(), Some(a.to_string())),
        ];
        for (update, stored, expected) in cases {
            assert_eq!(
                update.homeserver_after(stored.as_ref()),
                expected,
                "{update:?}"
            );
        }
    }
}
