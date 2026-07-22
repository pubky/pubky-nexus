//! Neo4j/GDS-backed implementation of [`TrustRankEngine`], plus its Cypher
//! queries. All Neo4j specifics live here; the abstraction is in `super::engine`.

pub mod queries;

use std::time::Duration;

use async_trait::async_trait;
use nexus_common::db::fetch_row_from_graph;
use nexus_common::types::DynError;
use tracing::{info, warn};

use super::engine::{TrustRankEngine, TrustRankParams, TrustRankStats};
use queries::{
    count_matching_seeds, drop_stale_trust_graphs, drop_trust_graph, project_trust_graph,
    trust_rank_pagerank_write, TRUST_GRAPH_PREFIX,
};

/// Computes trust rank via the Neo4j GDS `pageRank` procedure in write mode,
/// storing the result on each `:User` node as the `trust` property.
///
/// Seeds are weighted equally (`v` uniform, `1/N` over `params.seed_ids`): GDS
/// 2.13 (the only version on Neo4j 5.26) has no weighted `sourceNodes`. The GDS
/// projection is rebuilt each run — Neo4j Community has no incremental update.
#[derive(Debug, Clone, Copy)]
pub struct GdsNeo4j {
    /// L1-normalize the written scores into a distribution (summing to 1). On in
    /// production; tests turn it off to observe the raw scores, which expose the
    /// mass a reachable dangling node leaks (that L1Norm otherwise rescales away).
    use_l1norm: bool,
    /// GDS projections older than this are swept as crash leftovers.
    stale_graph_age: Duration,
}

impl GdsNeo4j {
    /// Engine with an explicit scaling choice (`use_l1norm`: `L1Norm` in
    /// production, `false` in tests to observe raw scores) and the stale-graph
    /// sweep age (see [`GdsNeo4j::stale_graph_age`]).
    pub fn new(use_l1norm: bool, stale_graph_age: Duration) -> Self {
        Self {
            use_l1norm,
            stale_graph_age,
        }
    }
}

#[async_trait]
impl TrustRankEngine for GdsNeo4j {
    async fn compute(&self, params: &TrustRankParams) -> Result<TrustRankStats, DynError> {
        if params.seed_ids.is_empty() {
            return Err(
                "trust_rank.seed is empty; refusing to compute trust rank without a seed set"
                    .into(),
            );
        }

        let damping_factor = 1.0 - params.alpha;

        // No matching seed → GDS gets an empty sourceNodes list, treats it as
        // unset, and computes non-personalized (Sybil-vulnerable) PageRank.
        // Fail fast instead.
        let matched: i64 = fetch_row_from_graph(count_matching_seeds(&params.seed_ids))
            .await?
            .map(|row| row.get("matched"))
            .transpose()?
            .unwrap_or(0);
        let configured = params.seed_ids.len() as i64;
        if matched == 0 {
            return Err(format!(
                "none of the {configured} configured trust seeds exist as users in the graph; refusing to compute trust rank"
            )
            .into());
        }
        if matched < configured {
            warn!(
                matched,
                configured, "Some configured trust seeds do not exist in the graph"
            );
        }

        // Per-run unique name so a concurrent run can't clobber this
        // run's in-flight projection.
        let graph_name = format!(
            "{TRUST_GRAPH_PREFIX}-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_millis()
        );

        // Sweep projections leaked by crashed runs (age-gated, can't race a live
        // run). Catalog calls use fetch_row_from_graph (PULL): GDS runs the side
        // effect only when the row is pulled.
        let stale_secs = self.stale_graph_age.as_secs() as i64;
        if let Some(row) = fetch_row_from_graph(drop_stale_trust_graphs(stale_secs)).await? {
            let dropped: Vec<String> = row.get("dropped").unwrap_or_default();
            if !dropped.is_empty() {
                warn!(
                    ?dropped,
                    "Dropped stale trust graph projections left by crashed runs"
                );
            }
        }

        let projected = fetch_row_from_graph(project_trust_graph(&graph_name)).await?;
        if let Some(row) = &projected {
            let node_count: i64 = row.get("nodeCount").unwrap_or_default();
            let relationship_count: i64 = row.get("relationshipCount").unwrap_or_default();
            info!(
                node_count,
                relationship_count, "Projected trust graph for GDS PageRank"
            );
        }

        let write_result = fetch_row_from_graph(trust_rank_pagerank_write(
            &graph_name,
            &params.seed_ids,
            damping_factor,
            params.max_iterations,
            params.tolerance,
            if self.use_l1norm { "L1Norm" } else { "NONE" },
        ))
        .await;

        // Always drop the projection so a failed run doesn't leak GDS memory.
        // Log-and-continue: a `?` would shadow the pagerank error; the stale
        // sweep reclaims any leak on a later run.
        if let Err(e) = fetch_row_from_graph(drop_trust_graph(&graph_name)).await {
            warn!(
                graph_name,
                "Failed to drop trust graph projection after run: {e:?}"
            );
        }

        let row = write_result?
            .ok_or_else(|| -> DynError { "GDS pageRank.write returned no summary row".into() })?;
        let users_written: i64 = row.get("nodePropertiesWritten").unwrap_or_default();
        let ran_iterations: i64 = row.get("ranIterations").unwrap_or_default();
        let did_converge: bool = row.get("didConverge").unwrap_or_default();

        if users_written == 0 {
            return Err(
                "trust rank computation wrote no scores; check the seed set and follow graph"
                    .into(),
            );
        }

        info!(users_written, did_converge, "Trust rank recomputed");

        Ok(TrustRankStats {
            users_written: users_written as u64,
            ran_iterations: ran_iterations as u64,
            did_converge,
        })
    }
}
