//! # User Homeserver Resolver
//!
//! Periodic task that resolves each user's homeserver and persists
//! the `(:User)-[:HOSTED_BY]->(:Homeserver)` relationship in Neo4j.

use nexus_common::db::{
    exec_single_row, fetch_key_from_graph, queries, GraphResult, PubkyConnector,
};
use nexus_common::types::DynError;

use pubky::PublicKey;
use pubky_app_specs::PubkyId;
use tracing::{debug, warn};

/// Main entry point for one cycle of the periodic task.
///
/// `ttl_ms` controls the minimum time before a user's mapping is re-resolved.
/// Users whose `HOSTED_BY.resolved_at` is newer than `ttl_ms` are skipped.
pub async fn run(ttl_ms: u64) -> Result<(), DynError> {
    let user_ids = get_users_needing_resolution(ttl_ms).await?;
    if user_ids.is_empty() {
        debug!("No users need homeserver resolution");
        return Ok(());
    }
    debug!("Resolving homeservers for {} users", user_ids.len());

    for user_id in &user_ids {
        if let Err(e) = resolve_user(user_id).await {
            warn!("Failed to resolve homeserver for user {user_id}: {e}");
        }
    }

    Ok(())
}

/// Fetches user IDs whose homeserver mapping is stale or missing.
///
/// A mapping is considered stale when its `resolved_at` timestamp is older
/// than `ttl_ms` milliseconds ago.
async fn get_users_needing_resolution(ttl_ms: u64) -> GraphResult<Vec<String>> {
    let query = queries::get::get_users_needing_hs_resolution(ttl_ms);
    let maybe_user_ids = fetch_key_from_graph(query, "user_ids").await?;
    Ok(maybe_user_ids.unwrap_or_default())
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
pub async fn get_user_ids_by_homeserver(hs_id: &str) -> GraphResult<Vec<String>> {
    let query = queries::get::get_users_by_homeserver(hs_id);
    let maybe_user_ids = fetch_key_from_graph(query, "user_ids").await?;
    Ok(maybe_user_ids.unwrap_or_default())
}

// TODO Move tests to separate module? (switch to WatcherTest::setup())
#[cfg(test)]
mod tests {
    use super::*;
    use nexus_common::db::exec_single_row;
    use nexus_common::db::graph::Query;
    use nexus_common::types::DynError;
    use nexus_common::{StackConfig, StackManager};

    async fn setup() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await
    }

    /// Helper: create a User node in the graph
    async fn create_test_user(user_id: &str) -> GraphResult<()> {
        let query = Query::new(
            "create_test_user",
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
    async fn test_get_users_needing_resolution_ttl() -> Result<(), DynError> {
        setup().await?;

        let user_fresh = "ttl_test_user_fresh";
        let user_stale = "ttl_test_user_stale";
        let user_no_hs = "ttl_test_user_no_hs";
        let hs_id = "ttl_test_hs";

        create_test_user(user_fresh).await?;
        create_test_user(user_stale).await?;
        create_test_user(user_no_hs).await?;

        // Give user_fresh a recently resolved mapping
        exec_single_row(queries::put::set_user_homeserver(user_fresh, hs_id)).await?;

        // Give user_stale a mapping with an old resolved_at (1 hour ago)
        let stale_query = Query::new(
            "set_stale_hs",
            "MATCH (u:User {id: $user_id})
             MERGE (hs:Homeserver {id: $hs_id})
             MERGE (u)-[r:HOSTED_BY]->(hs)
             SET r.resolved_at = timestamp() - 7200000",
        )
        .param("user_id", user_stale)
        .param("hs_id", hs_id);
        exec_single_row(stale_query).await?;

        // user_no_hs has no HOSTED_BY at all

        // With a 1-hour TTL: user_fresh should be skipped, user_stale and user_no_hs returned
        let mut needing = get_users_needing_resolution(3_600_000).await?;
        needing.sort();

        assert!(
            !needing.contains(&user_fresh.to_string()),
            "Recently resolved user should be skipped"
        );
        assert!(
            needing.contains(&user_stale.to_string()),
            "Stale user should need resolution"
        );
        assert!(
            needing.contains(&user_no_hs.to_string()),
            "User without HOSTED_BY should need resolution"
        );

        // Cleanup
        cleanup_test_user(user_fresh).await?;
        cleanup_test_user(user_stale).await?;
        cleanup_test_user(user_no_hs).await?;

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
