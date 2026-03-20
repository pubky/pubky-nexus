//! # User Homeserver Resolver
//!
//! Periodic task that resolves each user's homeserver and persists
//! the `(:User)-[:HOSTED_BY]->(:Homeserver)` relationship in Neo4j.
//!
//! On resolution failure the task increments a per-user failure counter
//! (stored as a score in the `Sorted:Users:HsResolutionFailures` Redis
//! Sorted Set) and skips to the next user.
//! Users with more failures are processed last so that healthy users are
//! resolved first.

use std::collections::HashMap;

use nexus_common::db::kv::{RedisResult, SortOrder};
use nexus_common::db::{
    exec_single_row, fetch_key_from_graph, queries, GraphResult, PubkyConnector, RedisOps,
};
use nexus_common::types::DynError;

use pubky::PublicKey;
use pubky_app_specs::PubkyId;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, warn};

// ---------------------------------------------------------------------------
// Redis Sorted Set for the per-user failure counter
// ---------------------------------------------------------------------------

/// Key parts for the Sorted Set holding user PK as a member and the failure count as the score.
const USER_HS_FAILURES_KEY_PARTS: [&str; 2] = ["Users", "HsResolutionFailures"];

/// Tracks consecutive failed homeserver lookups for users via a Redis Sorted Set.
#[derive(Serialize, Deserialize)]
pub struct UserHsFailures;

impl RedisOps for UserHsFailures {}

impl UserHsFailures {
    /// Reads the failure count for a user (returns 0 when absent).
    #[allow(dead_code)]
    async fn get(user_id: &str) -> RedisResult<u64> {
        let score =
            Self::check_sorted_set_member(None, &USER_HS_FAILURES_KEY_PARTS, &[user_id]).await?;
        Ok(score.map_or(0, |s| s as u64))
    }

    /// Returns all failure counters as a map of user_id → failure_count.
    async fn get_all() -> RedisResult<HashMap<String, f64>> {
        let entries = Self::try_from_index_sorted_set(
            &USER_HS_FAILURES_KEY_PARTS,
            None,
            None,
            None,
            None,
            SortOrder::Ascending,
            None,
        )
        .await?;

        Ok(entries.unwrap_or_default().into_iter().collect())
    }

    /// Increments the failure counter by 1.
    async fn increment(user_id: &str) -> RedisResult<()> {
        Self::increment_score_index_sorted_set(&USER_HS_FAILURES_KEY_PARTS, &[user_id]).await
    }

    /// Removes the failure counter (called on success).
    async fn remove(user_id: &str) -> RedisResult<()> {
        Self::remove_from_index_sorted_set(None, &USER_HS_FAILURES_KEY_PARTS, &[user_id]).await
    }
}

// ---------------------------------------------------------------------------
// Core resolver logic
// ---------------------------------------------------------------------------

/// Fetches all user IDs from the graph.
async fn get_all_user_ids() -> GraphResult<Vec<String>> {
    let query = queries::get::get_all_user_ids();
    let maybe_user_ids = fetch_key_from_graph(query, "user_ids").await?;
    Ok(maybe_user_ids.unwrap_or_default())
}

/// Sorts user IDs by their failure count (ascending — fewest failures first).
/// Ties are broken by `user_id` ascending for deterministic ordering.
/// Returns `(user_id, failure_score)` pairs.
async fn sort_by_failures(user_ids: Vec<String>) -> RedisResult<Vec<(String, f64)>> {
    let failure_map = UserHsFailures::get_all().await?;

    let mut sorted_users_and_failures = user_ids
        .into_iter()
        .map(|id| (id.clone(), failure_map.get(&id).copied().unwrap_or(0.0)))
        .collect::<Vec<_>>();

    sorted_users_and_failures.sort_by(|(a_id, a_score), (b_id, b_score)| {
        a_score.total_cmp(b_score).then_with(|| a_id.cmp(b_id))
    });

    Ok(sorted_users_and_failures)
}

/// Resolves a single user's homeserver and persists the HOSTED_BY relationship.
async fn resolve_user(user_id: &str) -> Result<(), DynError> {
    let pubky = PubkyConnector::get()?;

    let user_pk = user_id.parse::<PublicKey>()?;
    let Some(hs_pk) = pubky.get_homeserver_of(&user_pk).await else {
        return Err(format!("User {user_id} has no published homeserver").into());
    };

    let hs_id = PubkyId::try_from(&hs_pk.into_inner().to_z32())?;

    let query = queries::put::set_user_homeserver(user_id, &hs_id);
    exec_single_row(query).await?;

    debug!("User {user_id} -> homeserver {hs_id}");
    Ok(())
}

/// Returns all user IDs hosted on a given homeserver.
/// TODO Should be used in [EventProcessor::poll_events]
pub async fn get_user_ids_by_homeserver(hs_id: &str) -> GraphResult<Vec<String>> {
    let query = queries::get::get_users_by_homeserver(hs_id);
    let maybe_user_ids = fetch_key_from_graph(query, "user_ids").await?;
    Ok(maybe_user_ids.unwrap_or_default())
}

/// Main entry point for one cycle of the periodic task.
pub async fn run() -> Result<(), DynError> {
    let user_ids = get_all_user_ids().await?;
    if user_ids.is_empty() {
        debug!("No users to resolve homeservers for");
        return Ok(());
    }

    let users_and_failures = sort_by_failures(user_ids).await?;
    debug!("Resolving HSs for {} users", users_and_failures.len());

    for (user_id, failures) in &users_and_failures {
        match resolve_user(user_id).await {
            Ok(_) => {
                if *failures > 0.0 {
                    UserHsFailures::remove(user_id).await.ok();
                }
            }
            Err(e) => {
                warn!("Failed to resolve homeserver for user {user_id}: {e}");
                if let Err(incr_err) = UserHsFailures::increment(user_id).await {
                    error!("Failed to increment failure counter for {user_id}: {incr_err}");
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_common::db::exec_single_row;
    use nexus_common::types::DynError;
    use nexus_common::{StackConfig, StackManager};

    async fn setup() -> Result<(), DynError> {
        StackManager::setup("unit-hs-resolver-test", &StackConfig::default()).await?;
        Ok(())
    }

    /// Helper: create a User node in the graph
    async fn create_test_user(user_id: &str) -> GraphResult<()> {
        let query = neo4rs::query(
            "MERGE (u:User {id: $id})
             SET u.name = 'test', u.indexed_at = 0
             RETURN u;",
        )
        .param("id", user_id);
        exec_single_row(query).await
    }

    /// Helper: clean up test data
    async fn cleanup_test_user(user_id: &str) -> GraphResult<()> {
        let query = queries::del::delete_user(user_id);
        exec_single_row(query).await
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_user_hs_failures_increment_and_remove() -> Result<(), DynError> {
        setup().await?;

        let user_id = "test_hs_failures_user_001";

        // Initially absent from the map
        assert_eq!(UserHsFailures::get(user_id).await?, 0);

        // Increment twice
        UserHsFailures::increment(user_id).await?;
        assert_eq!(UserHsFailures::get(user_id).await?, 1);
        UserHsFailures::increment(user_id).await?;
        assert_eq!(UserHsFailures::get(user_id).await?, 2);

        // Remove
        UserHsFailures::remove(user_id).await?;
        assert_eq!(UserHsFailures::get(user_id).await?, 0);

        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_sort_by_failures() -> Result<(), DynError> {
        setup().await?;

        let ids = vec![
            "sort_test_user_a".to_string(),
            "sort_test_user_b".to_string(),
            "sort_test_user_c".to_string(),
        ];

        // Set different failure counts
        // b=3, a=1, c=0
        UserHsFailures::increment("sort_test_user_a").await?;
        UserHsFailures::increment("sort_test_user_b").await?;
        UserHsFailures::increment("sort_test_user_b").await?;
        UserHsFailures::increment("sort_test_user_b").await?;

        let sorted = sort_by_failures(ids).await?;
        assert_eq!(sorted[0], ("sort_test_user_c".to_string(), 0.0)); // 0 failures
        assert_eq!(sorted[1], ("sort_test_user_a".to_string(), 1.0)); // 1 failure
        assert_eq!(sorted[2], ("sort_test_user_b".to_string(), 3.0)); // 3 failures

        // Cleanup
        UserHsFailures::remove("sort_test_user_a").await?;
        UserHsFailures::remove("sort_test_user_b").await?;

        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_set_user_homeserver_graph_query() -> Result<(), DynError> {
        setup().await?;

        let user_id = "hs_resolver_test_user_001";
        let hs_id_a = "hs_resolver_test_hs_aaa";
        let hs_id_b = "hs_resolver_test_hs_bbb";

        create_test_user(user_id).await?;

        // Set initial homeserver
        let query = queries::put::set_user_homeserver(user_id, hs_id_a);
        exec_single_row(query).await?;

        // Switch to a different homeserver
        let query = queries::put::set_user_homeserver(user_id, hs_id_b);
        exec_single_row(query).await?;

        // Cleanup
        cleanup_test_user(user_id).await?;

        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_set_user_homeserver_idempotent() -> Result<(), DynError> {
        setup().await?;

        let user_id = "hs_resolver_test_user_noop";
        let hs_id = "hs_resolver_test_hs_noop";

        create_test_user(user_id).await?;

        // Set homeserver for the first time
        let query = queries::put::set_user_homeserver(user_id, hs_id);
        exec_single_row(query).await?;

        // Set same homeserver again (should reuse HS, e.g. not create any orphan HS)
        let query = queries::put::set_user_homeserver(user_id, hs_id);
        exec_single_row(query).await?;

        // Cleanup
        cleanup_test_user(user_id).await?;

        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_get_user_ids_by_homeserver() -> Result<(), DynError> {
        setup().await?;

        let user_a = "hs_users_test_user_aaa";
        let user_b = "hs_users_test_user_bbb";
        let user_c = "hs_users_test_user_ccc";
        let hs_one = "hs_users_test_hs_one";
        let hs_two = "hs_users_test_hs_two";

        create_test_user(user_a).await?;
        create_test_user(user_b).await?;
        create_test_user(user_c).await?;

        // Host user_a and user_b on hs_one, user_c on hs_two
        exec_single_row(queries::put::set_user_homeserver(user_a, hs_one)).await?;
        exec_single_row(queries::put::set_user_homeserver(user_b, hs_one)).await?;
        exec_single_row(queries::put::set_user_homeserver(user_c, hs_two)).await?;

        // Query users on hs_one
        let mut users = get_user_ids_by_homeserver(hs_one).await?;
        users.sort();
        assert_eq!(users, vec![user_a, user_b]);

        // Query users on hs_two
        let users = get_user_ids_by_homeserver(hs_two).await?;
        assert_eq!(users, vec![user_c]);

        // Query unknown HS returns empty
        let users = get_user_ids_by_homeserver("nonexistent_hs").await?;
        assert!(users.is_empty());

        // Cleanup
        cleanup_test_user(user_a).await?;
        cleanup_test_user(user_b).await?;
        cleanup_test_user(user_c).await?;

        Ok(())
    }
}
