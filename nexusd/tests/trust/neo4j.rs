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

    let err = compute_result
        .err()
        .expect("compute should fail with a tiny cap");
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

    let params = TrustRankParams {
        seed_ids: vec![seed.clone()],
        alpha: 0.35,
        max_iterations: 200,
        tolerance: 1e-7,
        max_projection_bytes: None,
    };

    // Read every score written onto the nodes before tearing them down, so the
    // assertions below run against a clean graph regardless of pass/panic. Both
    // scalings run in one test: `gds.pageRank.write` writes `trust` on *every*
    // projected user, so splitting them into two tests would let concurrent runs
    // clobber each other's nodes (production serializes via the job lock).
    // This test never asserts on the stale-projection sweep; a long age keeps it
    // from touching any live projection so the value is otherwise irrelevant.
    let sweep_age = Duration::from_secs(3600);
    let read_result = async {
        // L1Norm (production scaling): the shape a seeded ranking must have.
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

        // Raw (L1Norm off): same run un-normalized, to observe the mass leak.
        GdsNeo4j::new(false, sweep_age)
            .compute(&params)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let raw_seed = trust_of(&seed).await?.context("raw seed score")?;
        let raw_a = trust_of(&a).await?.context("raw a score")?;
        let raw_b = trust_of(&b).await?.context("raw b score")?;

        anyhow::Ok((
            stats, seed_score, a_score, b_score, c_score, all_scores, raw_seed, raw_a, raw_b,
        ))
    }
    .await;

    delete_users(&[&seed, &a, &b, &c]).await?;

    let (stats, seed_score, a_score, b_score, c_score, all_scores, raw_seed, raw_a, raw_b) =
        read_result?;

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

    // Scaler toggle. `b` is a dangling node (reachable, but follows nobody). GDS
    // drops — never teleport-redistributes — its mass, so with a single seed the
    // raw scores leak below 1; L1Norm rescales the same vector back to a
    // distribution summing to 1, hiding the leak.
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

    Ok(())
}

/// `read_scores(limit)` bounds results to `limit` rows, highest-first.
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
