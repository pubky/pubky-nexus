//! End-to-end tests for the GDS engine. Requires the docker stack.
//!
//! Every `trust` assertion lives in the one test below on purpose: a run writes
//! `trust` across the whole `:User` label, so two concurrent runs clobber each
//! other. One test means no nextest serialization to configure.
//!
//! In the lib target because the deletion phase drives the private `queries`.

use anyhow::{Context, Result};
use nexus_common::db::graph::Query;
use nexus_common::db::{exec_single_row, fetch_all_rows_from_graph, fetch_row_from_graph};
use nexus_common::{StackConfig, StackManager};
use std::collections::HashMap;
use std::time::Duration;

use super::queries::{
    drop_trust_graph, trust_rank_pagerank_mutate, write_back_trust_scores, TRUST_GRAPH_PREFIX,
};
use super::GdsNeo4j;
use crate::trust::{read_scores, TrustRankEngine, TrustRankParams};

/// Follow chain `seed -> a -> b` plus an isolated `c`. `label` is a run-unique
/// marker so the deletion phase can project just these nodes.
async fn create_follow_graph(label: &str, seed: &str, a: &str, b: &str, c: &str) -> Result<()> {
    let cypher = format!(
        "CREATE (s:User:{label} {{id: $seed, name: 'trusttest seed'}})
         CREATE (a:User:{label} {{id: $a, name: 'trusttest a'}})
         CREATE (b:User:{label} {{id: $b, name: 'trusttest b'}})
         CREATE (c:User:{label} {{id: $c, name: 'trusttest c'}})
         CREATE (s)-[:FOLLOWS]->(a)
         CREATE (a)-[:FOLLOWS]->(b)"
    );
    let query = Query::new("trusttest_create_follow_graph", cypher)
        .param("seed", seed.to_string())
        .param("a", a.to_string())
        .param("b", b.to_string())
        .param("c", c.to_string());
    exec_single_row(query).await?;
    Ok(())
}

/// Projects only `label`, keeping the deletion phase's counts exact.
fn project_scoped(graph_name: &str, label: &str) -> Query {
    let cypher = format!(
        "CALL gds.graph.project($graph_name, '{label}', 'FOLLOWS')
         YIELD graphName, nodeCount
         RETURN graphName, nodeCount"
    );
    Query::new("trusttest_project_scoped", cypher).param("graph_name", graph_name.to_string())
}

async fn delete_users(ids: &[&str]) -> Result<()> {
    let query = Query::new(
        "trusttest_delete_users",
        "MATCH (u:User) WHERE u.id IN $ids DETACH DELETE u",
    )
    .param("ids", ids.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    exec_single_row(query).await?;
    Ok(())
}

async fn trust_of(id: &str) -> Result<Option<f64>> {
    let query = Query::new(
        "trusttest_read_trust",
        "MATCH (u:User {id: $id}) RETURN u.trust AS trust",
    )
    .param("id", id.to_string());
    let rows = fetch_all_rows_from_graph(query).await?;
    match rows.first() {
        Some(row) => Ok(row.get("trust").ok()),
        None => Ok(None),
    }
}

/// One pass over the engine: L1Norm scaling, raw scaling, and a user deleted
/// after projection (which the write-back must skip, not abort on).
#[tokio_shared_rt::test(shared)]
async fn trust_recompute_scores_graph_and_survives_deleted_users() -> Result<()> {
    StackManager::setup(&StackConfig::default())
        .await
        .map_err(|e| anyhow::anyhow!("could not initialise the stack: {e:?}"))?;

    // Unique ids per run so parallel/repeat runs never collide with each other
    // or with any existing user in the shared graph.
    let tag = format!(
        "{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    );
    // Labels are structural, so interpolated; digits-only keeps it an identifier.
    let label = format!("TrustTest{}", tag.replace('-', ""));
    let seed = format!("trusttest-{tag}-seed");
    let a = format!("trusttest-{tag}-a");
    let b = format!("trusttest-{tag}-b");
    let c = format!("trusttest-{tag}-c");
    let scoped_graph = format!("{TRUST_GRAPH_PREFIX}-scoped-{tag}");

    create_follow_graph(&label, &seed, &a, &b, &c).await?;

    let params = TrustRankParams {
        seed_ids: vec![seed.clone()],
        alpha: 0.35,
        max_iterations: 200,
        tolerance: 1e-7,
        max_projection_bytes: None,
    };

    // This test never asserts on the stale-projection sweep; a long age keeps it
    // from touching any live projection so the value is otherwise irrelevant.
    let sweep_age = Duration::from_secs(3600);

    // Read scores back before teardown, so assertions run on a clean graph
    // regardless of pass/panic.
    let outcome = async {
        // -- Phase 1: L1Norm, the production scaling. ------------------------
        let stats = GdsNeo4j::new(true, sweep_age)
            .compute(&params)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let seed_score = trust_of(&seed).await?.context("seed has no trust score")?;
        let a_score = trust_of(&a)
            .await?
            .context("followee `a` has no trust score")?;
        let b_score = trust_of(&b)
            .await?
            .context("followee `b` has no trust score")?;
        // An unreachable node scores exactly 0; GDS may write 0.0 or leave the
        // property unset — both mean "no trust".
        let c_score = trust_of(&c).await?.unwrap_or(0.0);
        // Read-back path must surface all scores (limit > graph size).
        let all_scores: HashMap<String, f64> = read_scores(1000).await?.into_iter().collect();

        // -- Phase 2: same run un-normalized, to observe the mass leak. ------
        GdsNeo4j::new(false, sweep_age)
            .compute(&params)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let raw_seed = trust_of(&seed).await?.context("raw seed score")?;
        let raw_a = trust_of(&a).await?.context("raw a score")?;
        let raw_b = trust_of(&b).await?.context("raw b score")?;

        // -- Phase 3: a user deleted between projection and write-back. ------
        // Driven phase by phase (not via `compute`) so the delete lands inside
        // that window deterministically, on a label-scoped projection.
        fetch_row_from_graph(project_scoped(&scoped_graph, &label)).await?;
        delete_users(&[&b]).await?;

        let mutate_row = fetch_row_from_graph(trust_rank_pagerank_mutate(
            &scoped_graph,
            std::slice::from_ref(&seed),
            0.65,
            200,
            1e-7,
            "L1Norm",
        ))
        .await
        .context("mutate touches no :User node, so a deleted user cannot fail it")?
        .context("mutate summary row")?;
        let scores_computed: i64 = mutate_row.get("nodePropertiesWritten").unwrap_or_default();

        // The regression: this used to die with EntityNotFoundException.
        // Collected in full so the length below is the real result-set size.
        let write_back_rows = fetch_all_rows_from_graph(write_back_trust_scores(&scoped_graph))
            .await
            .context("write-back must skip the deleted user, not abort the run")?;
        let written: i64 = write_back_rows
            .first()
            .context("write-back summary row")?
            .get("written")
            .unwrap_or_default();

        let post_delete_seed = trust_of(&seed).await?;
        let deleted_score = trust_of(&b).await?;

        anyhow::Ok((
            stats,
            seed_score,
            a_score,
            b_score,
            c_score,
            all_scores,
            raw_seed,
            raw_a,
            raw_b,
            scores_computed,
            written,
            write_back_rows.len(),
            post_delete_seed,
            deleted_score,
        ))
    }
    .await;

    let _ = fetch_row_from_graph(drop_trust_graph(&scoped_graph)).await;
    delete_users(&[&seed, &a, &b, &c]).await?;

    let (
        stats,
        seed_score,
        a_score,
        b_score,
        c_score,
        all_scores,
        raw_seed,
        raw_a,
        raw_b,
        scores_computed,
        written,
        write_back_row_count,
        post_delete_seed,
        deleted_score,
    ) = outcome?;

    // -- Phase 1 assertions ------------------------------------------------
    assert!(
        stats.users_written > 0,
        "recompute should report writing at least one user"
    );
    assert!(
        seed_score > 0.0,
        "seed should have positive trust, got {seed_score}"
    );
    assert!(
        a_score > 0.0,
        "directly-followed node `a` should inherit trust, got {a_score}"
    );
    assert!(
        b_score > 0.0,
        "transitively-followed node `b` should inherit trust, got {b_score}"
    );
    // A node unreachable from the seed set gets zero trust — the whole point of
    // a *seeded* (Sybil-resistant) ranking versus plain global PageRank.
    assert_eq!(
        c_score, 0.0,
        "isolated node `c` should have zero trust, got {c_score}"
    );
    // Trust decays as it flows along the follow chain: seed > a > b.
    assert!(
        seed_score > a_score,
        "seed ({seed_score}) should outrank its followee `a` ({a_score})"
    );
    assert!(
        a_score > b_score,
        "`a` ({a_score}) should outrank the node it follows, `b` ({b_score})"
    );
    // The read-back helper returns the same scores it wrote.
    assert_eq!(all_scores.get(&seed).copied(), Some(seed_score));
    assert_eq!(all_scores.get(&a).copied(), Some(a_score));

    // -- Phase 2 assertions ------------------------------------------------
    // `b` is a dangling node (reachable, but follows nobody). GDS drops — never
    // teleport-redistributes — its mass, so with a single seed the raw scores
    // leak below 1; L1Norm rescales the same vector back to a distribution
    // summing to 1, hiding the leak.
    let raw_sum = raw_seed + raw_a + raw_b;
    assert!(
        raw_sum < 1.0 - 1e-6,
        "raw scores should leak below 1 at the dangling node, summed to {raw_sum}"
    );
    let l1_sum = seed_score + a_score + b_score;
    assert!(
        (l1_sum - 1.0).abs() < 1e-6,
        "l1norm scores should sum to 1, summed to {l1_sum}"
    );
    // The rescale is uniform: node ratios are unchanged, so L1Norm only hides the
    // leak — it does not re-flow the leaked mass to other nodes.
    let raw_ratio = raw_a / raw_seed;
    let l1_ratio = a_score / seed_score;
    assert!(
        (raw_ratio - l1_ratio).abs() < 1e-6,
        "a/seed ratio should survive scaling: raw {raw_ratio} vs l1norm {l1_ratio}"
    );

    // -- Phase 3 assertions ------------------------------------------------
    // Four scoped nodes, one deleted after the snapshot: one score can't land.
    assert_eq!(
        scores_computed, 4,
        "scoped projection should rank this run's four nodes, got {scores_computed}"
    );
    assert_eq!(
        written,
        scores_computed - 1,
        "write-back should persist every score but the deleted user's \
         (computed {scores_computed}, written {written})"
    );
    assert!(
        post_delete_seed.is_some_and(|s| s > 0.0),
        "surviving seed should still have been written a score, got {post_delete_seed:?}"
    );
    assert_eq!(
        deleted_score, None,
        "deleted user must not be recreated by the write-back"
    );
    // Scores stay server-side: sum() collapses the per-node stream in the query.
    assert_eq!(
        write_back_row_count, 1,
        "write-back must return a single aggregate row, not one per ranked user \
         ({scores_computed} were ranked)"
    );

    Ok(())
}
