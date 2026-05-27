//! # User Homeserver Resolver
//!
//! Periodic task that resolves each user's homeserver and persists
//! the `(:User)-[:HOSTED_BY]->(:Homeserver)` relationship in Neo4j.

use nexus_common::db::{
    exec_single_row, fetch_key_from_graph, queries, GraphResult, PubkyConnector,
};
use nexus_common::types::DynError;
use nexus_common::WatcherConfig;
use opentelemetry::global;
use opentelemetry::metrics::Histogram;
use pubky::PublicKey;
use pubky_app_specs::PubkyId;
use std::sync::LazyLock;
use tokio::sync::watch::Receiver;
use tracing::{debug, error, info, warn};

static HS_RESOLVER_METRICS: LazyLock<HsResolverMetrics> = LazyLock::new(HsResolverMetrics::new);

/// Resolves a user's currently published homeserver from PKDNS/DHT.
///
/// Abstracted behind a trait so the resolver loop can be driven with a mock in
/// tests instead of hitting the network.
#[async_trait::async_trait]
pub trait PkdnsHomeserverResolver: Send + Sync {
    /// Returns the homeserver published for `user_pk`, or `None` if none is
    /// currently published.
    async fn resolve_homeserver(&self, user_pk: &PublicKey) -> Result<Option<PubkyId>, DynError>;
}

/// Production resolver backed by the shared [`PubkyConnector`].
pub struct PubkyConnectorResolver;

#[async_trait::async_trait]
impl PkdnsHomeserverResolver for PubkyConnectorResolver {
    async fn resolve_homeserver(&self, user_pk: &PublicKey) -> Result<Option<PubkyId>, DynError> {
        let pubky = PubkyConnector::get()?;
        let Some(hs_pk) = pubky.get_homeserver_of(user_pk).await else {
            return Ok(None);
        };
        Ok(Some(PubkyId::try_from(&hs_pk.into_inner().to_z32())?))
    }
}

pub struct UserHsResolverRunner {
    resolver: Box<dyn PkdnsHomeserverResolver>,
    ttl_ms: u64,
    shutdown_rx: Receiver<bool>,
}

impl UserHsResolverRunner {
    pub fn from_config(
        config: &WatcherConfig,
        resolver: Box<dyn PkdnsHomeserverResolver>,
        shutdown_rx: Receiver<bool>,
    ) -> Self {
        Self {
            resolver,
            ttl_ms: config.hs_resolver_ttl,
            shutdown_rx,
        }
    }

    pub async fn run(&self) -> Result<(), DynError> {
        let mut shutdown_rx = self.shutdown_rx.clone();
        run(self.resolver.as_ref(), self.ttl_ms, &mut shutdown_rx).await
    }
}

/// Main entry point for one cycle of the periodic task.
///
/// `ttl_ms` controls the minimum time before a user's mapping is re-resolved.
/// Users whose `HOSTED_BY.resolved_at` is newer than `ttl_ms` are skipped.
///
/// `shutdown_rx` cancels the in-flight resolution on shutdown; cancelled users
/// get re-picked-up on the next run via TTL.
pub async fn run(
    resolver: &dyn PkdnsHomeserverResolver,
    ttl_ms: u64,
    shutdown_rx: &mut Receiver<bool>,
) -> Result<(), DynError> {
    let user_ids = get_users_needing_resolution(ttl_ms).await?;
    if user_ids.is_empty() {
        debug!("No users need homeserver resolution");
        HS_RESOLVER_METRICS.run_total.record(0, &[]);
        HS_RESOLVER_METRICS.run_failed.record(0, &[]);
        return Ok(());
    }

    // Convert user_ids to user_pks
    let user_pks: Vec<PublicKey> = user_ids
        .iter()
        .filter_map(|user_id| {
            // For the user_ids that fail to convert, we log an error message and skip them
            user_id
                .parse::<PublicKey>()
                .map_err(|e| error!("Failed to parse user_id {user_id}: {e}"))
                .ok()
        })
        .collect();

    let attempted = user_pks.len() as u64;
    debug!("Resolving homeservers for {} users", attempted);

    let mut failed = 0u64;
    let mut processed = 0u64;

    // As of pubky 0.7.0 parallel resolution is possible but unreliable. This was tried:
    // - with the singleton Pubky client (up to 10% unresolved nodes with 10 req. in parallel)
    // - with a relay-only Pubky client (up to 95% unresolved nodes with 10 req. in parallel)
    // "unresolved nodes" = no HS was found using `get_homeserver_of(&user_pk)` for users with a known HS.
    //
    // The most reliable method remains sequential querying.
    //
    // To minimize the chance that User PKs are too close to each other and therefore might hit
    // the same DHT node, which can cause that node to interpret this as spammy requests and therefore
    // fail / refuse to resolve some of the queries, we order the User PKs such that every new query lands
    // as far as possible from all previous queries in the PK keyspace.
    //
    // To achieve this, we use bisection ordering.
    for user_pk in bisection_order_user_pks(user_pks) {
        tokio::select! {
            biased;
            _ = shutdown_rx.changed() => {
                info!("Shutdown detected; HS resolver stopping after {processed}/{attempted} users");
                break;
            }
            result = resolve_user(resolver, &user_pk) => {
                if let Err(e) = result {
                    failed += 1;
                    warn!("Failed to resolve HS for user {}: {e}", user_pk.z32());
                }
                processed += 1;
            }
        }
    }

    HS_RESOLVER_METRICS.run_total.record(attempted, &[]);
    HS_RESOLVER_METRICS.run_failed.record(failed, &[]);

    Ok(())
}

// Bisection ordering sorts the User PKs such that every new PK is as far as possible from all
// previously queried PKs in the keyspace.
//
// The algorithm:
//   1. Sort all PKs lexicographically.
//   2. Reorder via BFS over the implicit binary-search-tree layout of the sorted array:
//      emit the midpoint of each interval, then recurse into left and right halves.
//
// For a sorted array [K0..=K7] this produces [K4, K2, K6, K1, K3, K5, K7, K0], ensuring
// each successive query lands as far as possible from all previous ones in the keyspace.
fn bisection_order_user_pks(unsorted_pks: Vec<PublicKey>) -> Vec<PublicKey> {
    let mut sorted_pks = unsorted_pks;
    sorted_pks.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));

    let n = sorted_pks.len();
    let mut bisection_result = Vec::with_capacity(n);
    // Each entry is a half-open interval [lo, hi) of the sorted slice to process.
    let mut queue = std::collections::VecDeque::new();
    if n > 0 {
        queue.push_back((0usize, n));
    }
    while let Some((lo, hi)) = queue.pop_front() {
        if lo >= hi {
            continue;
        }
        let mid = lo + (hi - lo) / 2;
        bisection_result.push(sorted_pks[mid].clone());
        queue.push_back((lo, mid));
        queue.push_back((mid + 1, hi));
    }
    bisection_result
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
///
/// Homeserver switching is not fully implemented, so the bound homeserver is
/// never changed once set:
/// - No stored mapping: store whatever the DHT resolves (if anything).
/// - Stored mapping, DHT still points at it: clear any `stale` flag (resume indexing).
/// - Stored mapping, DHT changed or returns nothing: mark the mapping `stale`
///   so the watcher stops indexing the user until the DHT realigns.
async fn resolve_user(
    resolver: &dyn PkdnsHomeserverResolver,
    user_pk: &PublicKey,
) -> Result<(), DynError> {
    let user_id = user_pk.z32();

    let resolved_hs_id = resolver.resolve_homeserver(user_pk).await?;

    let Some(stored_hs_id) = get_user_homeserver(&user_id).await? else {
        // First-time resolution: store whatever the DHT resolves.
        let Some(hs_id) = resolved_hs_id else {
            debug!("User {user_id} has no published homeserver");
            return Ok(());
        };
        exec_single_row(queries::put::set_user_homeserver(&user_id, &hs_id)).await?;
        debug!("User {user_id} -> HS {hs_id}");
        return Ok(());
    };

    // Already bound to a homeserver: toggle the stale flag instead of switching.
    let still_matches = resolved_hs_id
        .as_ref()
        .is_some_and(|hs_id| hs_id.as_ref() == stored_hs_id.as_str());
    if still_matches {
        exec_single_row(queries::put::set_user_homeserver_stale(&user_id, false)).await?;
        debug!("User {user_id} still hosted on {stored_hs_id}, mapping active");
    } else {
        exec_single_row(queries::put::set_user_homeserver_stale(&user_id, true)).await?;
        warn!(
            "User {user_id} homeserver changed or was removed (stored {stored_hs_id}); \
             switching unsupported, mapping marked stale and indexing paused"
        );
    }

    Ok(())
}

/// Returns the homeserver ID a user is currently assigned to, if any.
async fn get_user_homeserver(user_id: &str) -> GraphResult<Option<String>> {
    let query = queries::get::get_user_homeserver(user_id);
    fetch_key_from_graph(query, "homeserver_id").await
}

/// Returns all user IDs hosted on a given homeserver.
pub async fn get_user_ids_by_homeserver(hs_id: &str) -> GraphResult<Vec<String>> {
    let query = queries::get::get_users_by_homeserver(hs_id);
    let maybe_user_ids = fetch_key_from_graph(query, "user_ids").await?;
    Ok(maybe_user_ids.unwrap_or_default())
}

struct HsResolverMetrics {
    run_total: Histogram<u64>,
    run_failed: Histogram<u64>,
}

impl HsResolverMetrics {
    fn new() -> Self {
        let meter = global::meter("hs-resolver-meter");

        Self {
            run_total: meter
                .u64_histogram("nexus.task.hs-resolver.total")
                .with_description("Number of attempted HS resolutions in each resolver run")
                .build(),
            run_failed: meter
                .u64_histogram("nexus.task.hs-resolver.failed")
                .with_description("Number of failed HS resolutions in each resolver run")
                .build(),
        }
    }
}

// TODO Move tests to separate module? (switch to WatcherTest::setup())
#[cfg(test)]
mod tests {
    use super::*;
    use nexus_common::db::exec_single_row;
    use nexus_common::db::graph::Query;
    use nexus_common::types::DynError;
    use nexus_common::{StackConfig, StackManager};
    use pubky::Keypair;

    async fn setup() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await
    }

    /// Resolver stub returning a fixed PKDNS result, so `resolve_user` can be
    /// driven without touching the DHT.
    struct MockResolver {
        result: Option<PubkyId>,
    }

    #[async_trait::async_trait]
    impl PkdnsHomeserverResolver for MockResolver {
        async fn resolve_homeserver(
            &self,
            _user_pk: &PublicKey,
        ) -> Result<Option<PubkyId>, DynError> {
            Ok(self.result.clone())
        }
    }

    /// Helper: a random homeserver id.
    fn random_hs_id() -> PubkyId {
        PubkyId::from(Keypair::random().public_key())
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

    #[tokio_shared_rt::test(shared)]
    async fn test_get_user_homeserver() -> Result<(), DynError> {
        setup().await?;

        let user_id = "get_user_hs_test_user";
        let hs_id = "get_user_hs_test_hs";

        create_test_user(user_id).await?;

        // No HOSTED_BY edge yet
        assert_eq!(get_user_homeserver(user_id).await?, None);

        // After assignment the current homeserver is returned
        exec_single_row(queries::put::set_user_homeserver(user_id, hs_id)).await?;
        assert_eq!(get_user_homeserver(user_id).await?, Some(hs_id.to_string()));

        cleanup_test_user(user_id).await?;

        Ok(())
    }

    /// First-time resolution stores whatever the DHT resolves.
    #[tokio_shared_rt::test(shared)]
    async fn test_resolve_user_first_time_sets_homeserver() -> Result<(), DynError> {
        setup().await?;

        let user_pk = Keypair::random().public_key();
        let user_id = user_pk.z32();
        let hs_id = random_hs_id();

        create_test_user(&user_id).await?;

        let resolver = MockResolver {
            result: Some(hs_id.clone()),
        };
        resolve_user(&resolver, &user_pk).await?;

        assert_eq!(
            get_user_homeserver(&user_id).await?,
            Some(hs_id.to_string())
        );
        assert!(get_user_ids_by_homeserver(&hs_id).await?.contains(&user_id));

        cleanup_test_user(&user_id).await?;

        Ok(())
    }

    /// A user with no published homeserver and no stored mapping is left alone.
    #[tokio_shared_rt::test(shared)]
    async fn test_resolve_user_first_time_no_homeserver_noop() -> Result<(), DynError> {
        setup().await?;

        let user_pk = Keypair::random().public_key();
        let user_id = user_pk.z32();

        create_test_user(&user_id).await?;

        let resolver = MockResolver { result: None };
        resolve_user(&resolver, &user_pk).await?;

        assert_eq!(get_user_homeserver(&user_id).await?, None);

        cleanup_test_user(&user_id).await?;

        Ok(())
    }

    /// When the published homeserver changes, the binding is kept but marked
    /// stale so the user stops being indexed; the new homeserver is never set.
    #[tokio_shared_rt::test(shared)]
    async fn test_resolve_user_change_keeps_binding_and_marks_stale() -> Result<(), DynError> {
        setup().await?;

        let user_pk = Keypair::random().public_key();
        let user_id = user_pk.z32();
        let stored_hs = random_hs_id();
        let new_hs = random_hs_id();

        create_test_user(&user_id).await?;
        exec_single_row(queries::put::set_user_homeserver(&user_id, &stored_hs)).await?;

        // DHT now points at a different homeserver
        let resolver = MockResolver {
            result: Some(new_hs.clone()),
        };
        resolve_user(&resolver, &user_pk).await?;

        // Binding unchanged, and the user is indexed on neither homeserver
        assert_eq!(
            get_user_homeserver(&user_id).await?,
            Some(stored_hs.to_string())
        );
        assert!(!get_user_ids_by_homeserver(&stored_hs)
            .await?
            .contains(&user_id));
        assert!(!get_user_ids_by_homeserver(&new_hs)
            .await?
            .contains(&user_id));

        cleanup_test_user(&user_id).await?;

        Ok(())
    }

    /// When the homeserver is unpublished, the binding is kept but marked stale.
    #[tokio_shared_rt::test(shared)]
    async fn test_resolve_user_unpublished_keeps_binding_and_marks_stale() -> Result<(), DynError> {
        setup().await?;

        let user_pk = Keypair::random().public_key();
        let user_id = user_pk.z32();
        let stored_hs = random_hs_id();

        create_test_user(&user_id).await?;
        exec_single_row(queries::put::set_user_homeserver(&user_id, &stored_hs)).await?;

        // DHT no longer publishes a homeserver
        let resolver = MockResolver { result: None };
        resolve_user(&resolver, &user_pk).await?;

        assert_eq!(
            get_user_homeserver(&user_id).await?,
            Some(stored_hs.to_string())
        );
        assert!(!get_user_ids_by_homeserver(&stored_hs)
            .await?
            .contains(&user_id));

        cleanup_test_user(&user_id).await?;

        Ok(())
    }

    /// When the DHT realigns with the stored homeserver, the stale flag clears
    /// and the user is indexed again.
    #[tokio_shared_rt::test(shared)]
    async fn test_resolve_user_realign_clears_stale() -> Result<(), DynError> {
        setup().await?;

        let user_pk = Keypair::random().public_key();
        let user_id = user_pk.z32();
        let stored_hs = random_hs_id();

        create_test_user(&user_id).await?;
        exec_single_row(queries::put::set_user_homeserver(&user_id, &stored_hs)).await?;
        // Start from a stale mapping
        exec_single_row(queries::put::set_user_homeserver_stale(&user_id, true)).await?;
        assert!(!get_user_ids_by_homeserver(&stored_hs)
            .await?
            .contains(&user_id));

        // DHT points back at the stored homeserver
        let resolver = MockResolver {
            result: Some(stored_hs.clone()),
        };
        resolve_user(&resolver, &user_pk).await?;

        assert!(get_user_ids_by_homeserver(&stored_hs)
            .await?
            .contains(&user_id));

        cleanup_test_user(&user_id).await?;

        Ok(())
    }

    #[test]
    fn test_bisection_order() {
        // Empty and single-element edge cases.
        assert!(bisection_order_user_pks(vec![]).is_empty());
        let lone = Keypair::random().public_key();
        let result = bisection_order_user_pks(vec![lone.clone()]);
        assert_eq!(result[0].as_bytes(), lone.as_bytes());

        // For 8 keys, verify the full BFS-bisection permutation.
        // Sort first to establish the ground-truth lexicographic order.
        let mut sorted: Vec<PublicKey> = (0..8).map(|_| Keypair::random().public_key()).collect();
        sorted.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));

        // BFS over a sorted array of 8 elements (half-open intervals) emits:
        //   (0,8)→4, (0,4)→2, (5,8)→6, (0,2)→1, (3,4)→3, (5,6)→5, (7,8)→7, (0,1)→0
        let expected_indices: [usize; 8] = [4, 2, 6, 1, 3, 5, 7, 0];

        let result = bisection_order_user_pks(sorted.clone());
        assert_eq!(result.len(), 8);
        for (pos, &idx) in expected_indices.iter().enumerate() {
            assert_eq!(
                result[pos].as_bytes(),
                sorted[idx].as_bytes(),
                "position {pos}: expected sorted[{idx}]"
            );
        }
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_remove_user_homeserver() -> Result<(), DynError> {
        setup().await?;

        let user_id = "remove_hosted_by_test_user";
        let hs_id = "remove_hosted_by_test_hs";

        create_test_user(user_id).await?;

        // Assign a homeserver
        exec_single_row(queries::put::set_user_homeserver(user_id, hs_id)).await?;
        let users = get_user_ids_by_homeserver(hs_id).await?;
        assert!(
            users.contains(&user_id.to_string()),
            "user should be hosted"
        );

        // Remove the HOSTED_BY edge
        exec_single_row(queries::del::remove_user_homeserver(user_id)).await?;

        // User is no longer listed on that homeserver
        let users = get_user_ids_by_homeserver(hs_id).await?;
        assert!(
            !users.contains(&user_id.to_string()),
            "user should no longer be hosted after removal"
        );

        // User now needs resolution again (no HOSTED_BY edge)
        let needing = get_users_needing_resolution(3_600_000).await?;
        assert!(
            needing.contains(&user_id.to_string()),
            "user without HOSTED_BY should need resolution"
        );

        // Removing again is a no-op (no error)
        exec_single_row(queries::del::remove_user_homeserver(user_id)).await?;

        // Cleanup
        cleanup_test_user(user_id).await?;

        Ok(())
    }
}
