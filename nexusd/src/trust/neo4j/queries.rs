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

/// Age gate for the stale-projection sweep, in hours. Set to 2× the run
/// deadline (`MAX_RUN`, 1h) so the sweep can never race a lock-holding run.
const STALE_GRAPH_SWEEP_HOURS: i64 = 2;

/// Reclaims GDS projections left by a run that died before dropping its own:
/// prefix-matched entries older than [`STALE_GRAPH_SWEEP_HOURS`]. A run drops
/// its projection on success and on error, so a leak means a hard crash
/// (SIGKILL/OOM) or a shutdown-cancelled run.
///
/// Age proxies for "the owner is dead". The real invariant comes from the run
/// lock: a run is abandoned at its deadline (`nexusd::jobs::lock::MAX_RUN`, 1h)
/// and its lease expires shortly after, after which a second run may start. The
/// sweep gate is 2× that deadline, comfortably past the lease, so a projection
/// old enough to sweep cannot belong to a run still holding (or able to hold)
/// the lock — the sweep never races a live computation. If `MAX_RUN` changes,
/// revisit this margin.
pub fn drop_stale_trust_graphs() -> Query {
    Query::new(
        "drop_stale_trust_graphs",
        "CALL gds.graph.list() YIELD graphName, creationTime
         WHERE graphName STARTS WITH $prefix AND creationTime < datetime() - duration({hours: $stale_hours})
         CALL gds.graph.drop(graphName, false) YIELD graphName AS dropped
         RETURN collect(dropped) AS dropped",
    )
    .param("prefix", TRUST_GRAPH_PREFIX)
    .param("stale_hours", STALE_GRAPH_SWEEP_HOURS)
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
/// `scaler: 'L1Norm'` writes scores as an L1-normalized distribution (summing
/// to 1) rather than raw rank values. Write mode overwrites `trust` on every
/// projected `:User` each run, so scores never go stale.
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
             scaler: 'L1Norm'
         })
         YIELD nodePropertiesWritten, ranIterations, didConverge
         RETURN nodePropertiesWritten, ranIterations, didConverge",
    )
    .param("graph_name", graph_name.to_string())
    .param("seed_ids", seed_ids.to_vec())
    .param("damping_factor", damping_factor)
    .param("max_iterations", max_iterations as i64)
    .param("tolerance", tolerance)
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
/// `OPTIONAL MATCH` keeps one row per requested id even if a user is missing;
/// each `COUNT {}` subquery mirrors `nexus_common`'s `user_counts` and stays
/// O(1) per field, as no row-multiplying match precedes them.
pub fn trust_report_user_details(user_ids: &[String]) -> Query {
    Query::new(
        "trust_report_user_details",
        "
        UNWIND $user_ids AS user_id
        OPTIONAL MATCH (u:User {id: user_id})
        RETURN
            user_id AS id,
            u.name AS name,
            COUNT { (u)-[:FOLLOWS]->(:User) } AS following,
            COUNT { (:User)-[:FOLLOWS]->(u) } AS followers,
            COUNT { (u)-[:FOLLOWS]->(friend:User) WHERE (friend)-[:FOLLOWS]->(u) } AS friends,
            COUNT { (u)-[:AUTHORED]->(:Post) } AS posts,
            COUNT { (u)-[:AUTHORED]->(:Post)-[:REPLIED]->(:Post) } AS replies,
            COUNT { (u)-[:AUTHORED]->(p:Post) WHERE p.kind = 'collection' } AS collections,
            COUNT { (u)-[:BOOKMARKED]->(bp:Post) WHERE (bp.kind IS NULL OR bp.kind <> 'collection') } AS bookmarks,
            COUNT { (u)-[:TAGGED]->(:User) } + COUNT { (u)-[:TAGGED]->(:Post) } AS tagged,
            COUNT { (:User)-[:TAGGED]->(u) } AS tags,
            COUNT { MATCH (u)<-[t:TAGGED]-(:User) RETURN DISTINCT t.label } AS unique_tags
        ",
    )
    .param("user_ids", user_ids.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_rank_pagerank_write_populates_seed_ids() {
        let seed_ids = vec!["alice".to_string(), "bob".to_string()];
        let q = trust_rank_pagerank_write("nexusTrustGraph-1-2", &seed_ids, 0.65, 200, 0.0000001);
        let populated = q.to_cypher_populated();
        assert!(populated.contains("['alice', 'bob']"));
        assert!(populated.contains("0.65"));
        assert!(populated.contains("'nexusTrustGraph-1-2'"));
        assert!(populated.contains("writeProperty: 'trust'"));
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
}
