use nexus_common::db::graph::queries::get::user_count_fields_columns;
use nexus_common::db::graph::Query;

/// Name prefix of the per-run GDS graph projections. Each run projects under a
/// unique `{prefix}-{pid}-{millis}` name so concurrent runs can't clobber each
/// other; crash leftovers are swept by [`drop_stale_trust_graphs`].
pub const TRUST_GRAPH_PREFIX: &str = "nexusTrustGraph";

/// Drops the named GDS graph projection if it exists; a no-op otherwise (`failIfMissing: false`).
pub fn drop_trust_graph(graph_name: &str) -> Query {
    Query::new(
        "drop_trust_graph",
        "CALL gds.graph.drop($graph_name, false) YIELD graphName RETURN graphName",
    )
    .param("graph_name", graph_name.to_string())
}

/// Reclaims GDS projections left by a run that died before dropping its own:
/// prefix-matched entries older than `stale_seconds`. A run drops its projection
/// on success and on error, so a leak means a hard crash (SIGKILL/OOM) or a
/// shutdown-cancelled run.
///
/// Age proxies for "the owner is dead". The caller sizes `stale_seconds` past a
/// run's lease expiry so a projection old enough to sweep cannot belong to a run
/// still holding (or able to hold) the lock.
pub fn drop_stale_trust_graphs(stale_seconds: i64) -> Query {
    Query::new(
        "drop_stale_trust_graphs",
        "CALL gds.graph.list() YIELD graphName, creationTime
         WHERE graphName STARTS WITH $prefix AND creationTime < datetime() - duration({seconds: $stale_seconds})
         CALL gds.graph.drop(graphName, false) YIELD graphName AS dropped
         RETURN collect(dropped) AS dropped",
    )
    .param("prefix", TRUST_GRAPH_PREFIX)
    .param("stale_seconds", stale_seconds)
}

/// Projects the follow graph (`User` nodes, `FOLLOWS` edges) into the GDS in-memory catalog.
pub fn project_trust_graph(graph_name: &str) -> Query {
    Query::new(
        "project_trust_graph",
        "CALL gds.graph.project($graph_name, 'User', 'FOLLOWS')
         YIELD graphName, nodeCount, relationshipCount
         RETURN graphName, nodeCount, relationshipCount",
    )
    .param("graph_name", graph_name.to_string())
}

/// Counts how many of the configured seed ids exist as `:User` nodes.
///
/// Run before the PageRank query: if zero seeds match, GDS gets an empty
/// `sourceNodes`, treats it as unset, and computes non-personalized PageRank
/// instead of the seeded trust ranking.
pub fn count_matching_seeds(seed_ids: &[String]) -> Query {
    Query::new(
        "count_matching_seeds",
        "MATCH (seed:User) WHERE seed.id IN $seed_ids RETURN count(seed) AS matched",
    )
    .param("seed_ids", seed_ids.to_vec())
}

/// Runs personalized PageRank over the projected follow graph in GDS **write
/// mode**, storing each score on its `:User` node as the `trust` property.
/// Teleports uniformly across `seed_ids` (`v = 1/N`); GDS 2.13 (the only
/// version on Neo4j 5.26) has no weighted `sourceNodes`.
///
/// `scaler` picks the GDS output scaling: `L1Norm` (production) writes an
/// L1-normalized distribution (summing to 1); `NONE` writes raw rank values.
/// GDS drops — never teleport-redistributes — the mass of reachable dangling
/// nodes, so raw single-seed scores sum to < 1; `L1Norm` rescales that back to a
/// distribution, hiding the leak. Write mode overwrites `trust` on every
/// projected `:User` each run; the projection is a snapshot, so users created
/// after it carry no score until the next run picks them up (normal for a batch
/// job).
///
/// The `size(sourceNodes) > 0` guard makes the query produce no rows when no
/// seed matches, so a raced seed deletion can never fall back to
/// non-personalized PageRank — the caller's "no summary row" path fires instead.
pub fn trust_rank_pagerank_write(
    graph_name: &str,
    seed_ids: &[String],
    damping_factor: f64,
    max_iterations: u32,
    tolerance: f64,
    scaler: &str,
) -> Query {
    Query::new(
        "trust_rank_pagerank_write",
        "MATCH (seed:User) WHERE seed.id IN $seed_ids
         WITH collect(seed) AS sourceNodes
         WHERE size(sourceNodes) > 0
         CALL gds.pageRank.write($graph_name, {
             writeProperty: 'trust',
             sourceNodes: sourceNodes,
             dampingFactor: $damping_factor,
             maxIterations: $max_iterations,
             tolerance: $tolerance,
             scaler: $scaler
         })
         YIELD nodePropertiesWritten, ranIterations, didConverge
         RETURN nodePropertiesWritten, ranIterations, didConverge",
    )
    .param("graph_name", graph_name.to_string())
    .param("seed_ids", seed_ids.to_vec())
    .param("damping_factor", damping_factor)
    .param("max_iterations", max_iterations as i64)
    .param("tolerance", tolerance)
    .param("scaler", scaler.to_string())
}

/// Reads back the `trust` scores written by [`trust_rank_pagerank_write`],
/// highest first. Off any hot path, so a full scan of scored users is fine.
pub fn read_trust_scores() -> Query {
    Query::new(
        "read_trust_scores",
        "MATCH (u:User) WHERE u.trust IS NOT NULL
         RETURN u.id AS user_id, u.trust AS score
         ORDER BY score DESC",
    )
}

/// Batched name + counts for a list of user ids, read straight from the graph
/// (no Redis) so the report's numbers reflect current state, not a stale cache.
///
/// The count columns come from `nexus_common`'s shared `USER_COUNT_FIELDS` (via
/// [`user_count_fields_columns`]), so the filters can't drift from `user_counts`.
/// `OPTIONAL MATCH` keeps one row per requested id even if a user is missing;
/// that match is single-row per id (not row-multiplying), so each `COUNT {}`
/// stays O(1) per field.
pub fn trust_report_user_details(user_ids: &[String]) -> Query {
    let cypher = format!(
        "UNWIND $user_ids AS user_id
         OPTIONAL MATCH (u:User {{id: user_id}})
         RETURN
             user_id AS id,
             u.name AS name,
             {fields}",
        fields = user_count_fields_columns()
    );
    Query::new("trust_report_user_details", cypher).param("user_ids", user_ids.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_rank_pagerank_write_populates_seed_ids() {
        let seed_ids = vec!["alice".to_string(), "bob".to_string()];
        let q = trust_rank_pagerank_write(
            "nexusTrustGraph-1-2",
            &seed_ids,
            0.65,
            200,
            0.0000001,
            "L1Norm",
        );
        let populated = q.to_cypher_populated();
        assert!(populated.contains("['alice', 'bob']"));
        assert!(populated.contains("0.65"));
        assert!(populated.contains("'nexusTrustGraph-1-2'"));
        assert!(populated.contains("writeProperty: 'trust'"));
        // Scaler is parameterized (toggled between L1Norm and NONE), not hardcoded.
        assert!(populated.contains("'L1Norm'"));
        // Guard closing the seed-deletion race; empty match → no rows, not global PageRank.
        assert!(populated.contains("size(sourceNodes) > 0"));
    }

    #[test]
    fn count_matching_seeds_populates_seed_ids() {
        let seed_ids = vec!["alice".to_string(), "bob".to_string()];
        let populated = count_matching_seeds(&seed_ids).to_cypher_populated();
        assert!(populated.contains("['alice', 'bob']"));
        assert!(populated.contains("count(seed) AS matched"));
    }

    // Smoke test: the trust report has no integration coverage (nexusd-side),
    // so just check the composed Cypher is well-formed off the OPTIONAL MATCH.
    #[test]
    fn trust_report_user_details_is_well_formed() {
        let cypher = trust_report_user_details(&["alice".to_string()]).to_cypher_populated();
        assert!(cypher.contains("OPTIONAL MATCH (u:User {id: user_id})"));
        assert!(cypher.contains("AS following"));
        assert!(cypher.contains("bp.kind <> 'collection'"));
    }
}
