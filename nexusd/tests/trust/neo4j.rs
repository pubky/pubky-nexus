//! Trust-rank tests that never complete a scoring run, so they run
//! concurrently: a run writes `trust` across the whole `:User` label, and two of
//! them clobber each other. Scoring users directly is fine — asserting on the
//! scores a run produced belongs in `src/trust/neo4j/tests.rs`.
//!
//! Requires the docker stack (Neo4j with the GDS plugin + Redis) to be up.

use anyhow::Result;
use nexus_common::db::exec_single_row;
use nexus_common::db::graph::Query;
use nexus_common::{StackConfig, StackManager};
use nexusd::trust::{read_scores, GdsNeo4j, TrustRankEngine, TrustRankParams};
use std::time::Duration;

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

/// `max_projection_bytes` cap aborts compute when the estimate exceeds the limit.
#[tokio_shared_rt::test(shared)]
async fn test_compute_aborts_when_estimate_exceeds_cap() -> Result<()> {
    StackManager::setup(&StackConfig::default())
        .await
        .map_err(|e| anyhow::anyhow!("could not initialise the stack: {e:?}"))?;

    let tag = format!(
        "{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    );
    let seed = format!("cap-{tag}-seed");
    let a = format!("cap-{tag}-a");
    let b = format!("cap-{tag}-b");
    let c = format!("cap-{tag}-c");

    create_follow_graph(&seed, &a, &b, &c).await?;

    let params = TrustRankParams {
        seed_ids: vec![seed.clone()],
        alpha: 0.35,
        max_iterations: 200,
        tolerance: 1e-7,
        max_projection_bytes: Some(1), // Deliberately tiny cap
    };

    let sweep_age = Duration::from_secs(3600);
    let compute_result = GdsNeo4j::new(true, sweep_age).compute(&params).await;

    delete_users(&[&seed, &a, &b, &c]).await?;

    let err = compute_result.expect_err("compute should fail with a tiny cap");
    let err_msg = format!("{err}");
    assert!(
        err_msg.contains("exceeds configured cap"),
        "error should mention cap violation, got: {err_msg}"
    );
    assert!(
        err_msg.contains("cap (1 bytes)"),
        "error should name the cap value, got: {err_msg}"
    );

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

/// Creates `n` scored users (`trust: 1.0..=n`) without GDS.
async fn create_scored_users(prefix: &str, n: usize) -> Result<()> {
    let query = Query::new(
        "trusttest_create_scored_users",
        "UNWIND range(1, $n) AS i
         CREATE (:User {id: $prefix + toString(i), trust: toFloat(i)})",
    )
    .param("prefix", prefix.to_string())
    .param("n", n as i64);
    exec_single_row(query).await?;
    Ok(())
}

/// Removes users by id prefix.
async fn delete_users_by_prefix(prefix: &str) -> Result<()> {
    let query = Query::new(
        "trusttest_delete_by_prefix",
        "MATCH (u:User) WHERE u.id STARTS WITH $prefix DETACH DELETE u",
    )
    .param("prefix", prefix.to_string());
    exec_single_row(query).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_read_scores_respects_limit() -> Result<()> {
    StackManager::setup(&StackConfig::default())
        .await
        .map_err(|e| anyhow::anyhow!("could not initialise the stack: {e:?}"))?;

    let prefix = format!(
        "trusttest-cap-{}-{}-",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    );

    // More users than limit to force row trimming.
    let limit = 5;
    create_scored_users(&prefix, limit + 3).await?;

    let read_result = read_scores(limit).await.map_err(|e| anyhow::anyhow!("{e}"));

    delete_users_by_prefix(&prefix).await?;

    let scores = read_result?;

    // LIMIT trims to exactly `limit` rows.
    assert_eq!(
        scores.len(),
        limit,
        "read_scores must return exactly `limit` rows when more scored users exist"
    );
    // Rows remain highest-first.
    assert!(
        scores.windows(2).all(|w| w[0].1 >= w[1].1),
        "capped scores must remain sorted highest-first, got {scores:?}"
    );

    Ok(())
}
