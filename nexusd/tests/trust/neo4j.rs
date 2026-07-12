//! Integration test for the Neo4j/GDS [`GdsNeo4j`] trust-rank engine.
//!
//! Builds a tiny, self-contained follow graph (so it doesn't depend on any
//! seeded fixtures), runs a real GDS PageRank recompute against the Neo4j
//! stack, and asserts the `trust` property is written back onto the `:User`
//! nodes with the shape a seeded/personalized ranking must have.
//!
//! Requires the docker stack (Neo4j with the GDS plugin + Redis) to be up.

use anyhow::{Context, Result};
use nexus_common::db::graph::Query;
use nexus_common::db::{exec_single_row, fetch_all_rows_from_graph};
use nexus_common::{StackConfig, StackManager};
use nexusd::trust::{read_scores, GdsNeo4j, TrustRankEngine, TrustRankParams};
use std::collections::HashMap;

/// Creates four `:User` nodes and the follow chain `seed -> a -> b`, leaving
/// `c` isolated (followed by no one, following no one).
async fn create_follow_graph(seed: &str, a: &str, b: &str, c: &str) -> Result<()> {
    let query = Query::new(
        "trusttest_create_follow_graph",
        "CREATE (s:User {id: $seed, name: 'trusttest seed'})
         CREATE (a:User {id: $a, name: 'trusttest a'})
         CREATE (b:User {id: $b, name: 'trusttest b'})
         CREATE (c:User {id: $c, name: 'trusttest c'})
         CREATE (s)-[:FOLLOWS]->(a)
         CREATE (a)-[:FOLLOWS]->(b)",
    )
    .param("seed", seed.to_string())
    .param("a", a.to_string())
    .param("b", b.to_string())
    .param("c", c.to_string());
    exec_single_row(query).await?;
    Ok(())
}

/// Removes the test nodes (and their edges) so the shared graph is left clean.
async fn delete_users(ids: &[&str]) -> Result<()> {
    let query = Query::new(
        "trusttest_delete_users",
        "MATCH (u:User) WHERE u.id IN $ids DETACH DELETE u",
    )
    .param("ids", ids.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    exec_single_row(query).await?;
    Ok(())
}

/// Reads a single user's `trust` property straight from the graph.
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

#[tokio_shared_rt::test(shared)]
async fn test_trust_recompute_assigns_scores_to_graph_nodes() -> Result<()> {
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
    let seed = format!("trusttest-{tag}-seed");
    let a = format!("trusttest-{tag}-a");
    let b = format!("trusttest-{tag}-b");
    let c = format!("trusttest-{tag}-c");

    create_follow_graph(&seed, &a, &b, &c).await?;

    // Recompute seeded PageRank teleporting only from `seed`.
    let params = TrustRankParams {
        seed_ids: vec![seed.clone()],
        alpha: 0.35,
        max_iterations: 200,
        tolerance: 1e-7,
    };
    let recompute_result = GdsNeo4j.compute(&params).await;

    // Read the scores that were written onto the nodes before tearing them
    // down, so the assertions below run against a clean graph regardless of
    // whether they pass or panic.
    let read_result = async {
        let stats = recompute_result.map_err(|e| anyhow::anyhow!("{e}"))?;
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
        // The full read-back path (used by the CLI/report) must also surface these.
        let all_scores: HashMap<String, f64> = read_scores().await?.into_iter().collect();
        anyhow::Ok((stats, seed_score, a_score, b_score, c_score, all_scores))
    }
    .await;

    delete_users(&[&seed, &a, &b, &c]).await?;

    let (stats, seed_score, a_score, b_score, c_score, all_scores) = read_result?;

    // Scores were actually written to the graph.
    assert!(
        stats.users_written > 0,
        "recompute should report writing at least one user"
    );

    // Seed and everything reachable from it along FOLLOWS gets positive trust.
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

    Ok(())
}
