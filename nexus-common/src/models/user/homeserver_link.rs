//! Where a user's content lives, for light clients.
//!
//! A light Nexus (#190) serves links rather than content, so a client fetches content from
//! each author's homeserver. Nexus already knows which homeserver that is (the `HOSTED_BY`
//! mapping the resolver keeps) and whether it answered the watcher's last poll, so it hands
//! both out instead of making every client look them up.

use crate::db::kv::RedisResult;
use crate::db::{fetch_all_rows_from_graph, queries, RedisOps};
use crate::models::error::ModelResult;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use utoipa::ToSchema;

/// Whether a homeserver answered the watcher's last poll.
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HomeserverReachability {
    /// The last poll succeeded.
    Ok,
    /// The last poll failed to reach it: transport failure or a 5xx answer.
    Unreachable,
}

/// The last [`HomeserverReachability`] the watcher observed for a homeserver, cached at
/// `Hs:Reachability:<homeserver_id>`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct HsReachability {
    pub reachability: HomeserverReachability,
    /// When it was observed, in milliseconds since the epoch.
    pub observed_at: i64,
}

impl RedisOps for HsReachability {}

impl HsReachability {
    /// Records what the watcher observed polling `homeserver_id`.
    pub async fn record(
        homeserver_id: &str,
        reachability: HomeserverReachability,
    ) -> RedisResult<()> {
        let observed = Self {
            reachability,
            observed_at: Utc::now().timestamp_millis(),
        };
        observed.put_index_json(&[homeserver_id], None, None).await
    }

    /// The last observation for each of `homeserver_ids`, in order; `None` where the
    /// watcher has not polled it yet.
    pub async fn get_many(homeserver_ids: &[&str]) -> RedisResult<Vec<Option<Self>>> {
        let keys: Vec<[&str; 1]> = homeserver_ids.iter().map(|id| [*id]).collect();
        let keys: Vec<&[&str]> = keys.iter().map(|key| key.as_slice()).collect();
        Self::try_from_index_multiple_json(&keys).await
    }
}

/// A user's homeserver, as a light client needs it to fetch the user's content.
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone, PartialEq)]
pub struct UserHomeserver {
    /// The homeserver's public key.
    pub id: String,
    /// The user may have moved: Nexus could not confirm this homeserver lately. Confirm it
    /// through pkarr before relying on it.
    pub stale: bool,
    /// Whether the homeserver answered when Nexus last polled it; `null` when Nexus has
    /// not polled it.
    pub status: Option<HomeserverReachability>,
}

impl UserHomeserver {
    /// The homeserver of each of `user_ids`, in order; `None` for a user with no known
    /// homeserver. One graph query and one Redis read for the whole list.
    pub async fn get_by_user_ids(user_ids: &[&str]) -> ModelResult<Vec<Option<Self>>> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = fetch_all_rows_from_graph(queries::get::get_users_homeservers(user_ids)).await?;
        let mut mappings: HashMap<String, (String, bool)> = HashMap::new();
        for row in rows {
            let user_id: String = row.get("user_id")?;
            if let Some(homeserver_id) = row.get::<Option<String>>("homeserver_id")? {
                let stale: bool = row.get("stale")?;
                mappings.insert(user_id, (homeserver_id, stale));
            }
        }

        let homeserver_ids: Vec<&str> = mappings
            .values()
            .map(|(homeserver_id, _)| homeserver_id.as_str())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let statuses: HashMap<&str, HomeserverReachability> = homeserver_ids
            .iter()
            .copied()
            .zip(HsReachability::get_many(&homeserver_ids).await?)
            .filter_map(|(id, observed)| observed.map(|observed| (id, observed.reachability)))
            .collect();

        Ok(user_ids
            .iter()
            .map(|user_id| {
                mappings.get(*user_id).map(|(homeserver_id, stale)| Self {
                    id: homeserver_id.clone(),
                    stale: *stale,
                    status: statuses.get(homeserver_id.as_str()).copied(),
                })
            })
            .collect())
    }
}
