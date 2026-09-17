//! Regression test for the Redis schema bootstrap (`setup_cache`). Requires the
//! docker stack (Redis with the query engine + Neo4j) to be up.
//!
//! `clear_redis()` issues FLUSHDB, which destroys the RediSearch index along
//! with the keys, so `setup_cache` must run again on every flush rather than
//! once per process. Wrapping it in a `OnceCell` (the way `setup_graph` is)
//! would pass every other test, because they boot the stack once and never
//! flush, while silently breaking `nexusd db clear --yes` and the reindex
//! bench. This test pins that behavior.
//!
//! It lives in its own binary and is the only nexusd test that flushes Redis.
//! `.config/nextest.toml` gives this binary `threads-required =
//! 'num-test-threads'` so it never overlaps with any other test in the run.
//! The cache is rebuilt with `reindex::sync()` before returning so a local
//! run leaves the mock data in place for whatever runs next.

use anyhow::{Context, Result};
use nexus_common::db::{get_redis_conn, kv::clear_redis, reindex, RedisOps};
use nexus_common::models::post::PostDetails;
use nexus_common::{StackConfig, StackManager};
use redis::Value;

const POST_CONTENT_INDEX: &str = "postContentIdx";

/// Flattens an FT.INFO reply into its leaf strings so assertions don't depend
/// on the exact nesting RediSearch uses for the `attributes` section.
fn flatten(value: Value, out: &mut Vec<String>) {
    match value {
        Value::Array(items) | Value::Set(items) => items.into_iter().for_each(|v| flatten(v, out)),
        Value::Map(pairs) => pairs.into_iter().for_each(|(k, v)| {
            flatten(k, out);
            flatten(v, out);
        }),
        Value::BulkString(bytes) => out.push(String::from_utf8_lossy(&bytes).into_owned()),
        Value::SimpleString(s) | Value::VerbatimString { text: s, .. } => out.push(s),
        Value::Int(i) => out.push(i.to_string()),
        _ => {}
    }
}

/// Returns every leaf string of `FT.INFO postContentIdx`, or the error the
/// server replied with (e.g. "Unknown index name" when the index is absent).
async fn post_content_index_info() -> Result<Vec<String>> {
    let mut conn = get_redis_conn().await?;
    let raw: Value = redis::cmd("FT.INFO")
        .arg(POST_CONTENT_INDEX)
        .query_async(&mut conn)
        .await
        .with_context(|| format!("FT.INFO {POST_CONTENT_INDEX} failed"))?;
    let mut leaves = Vec::new();
    flatten(raw, &mut leaves);
    Ok(leaves)
}

async fn db_size() -> Result<i64> {
    let mut conn = get_redis_conn().await?;
    Ok(redis::cmd("DBSIZE").query_async(&mut conn).await?)
}

fn assert_post_content_schema(info: &[String], prefix: &str, stage: &str) {
    for field in ["$.content", "$.author", "$.kind"] {
        assert!(
            info.iter().any(|s| s == field),
            "{stage}: {POST_CONTENT_INDEX} should index {field}, got {info:?}"
        );
    }
    assert!(
        info.iter().any(|s| s == prefix),
        "{stage}: {POST_CONTENT_INDEX} should be scoped to the {prefix} prefix, got {info:?}"
    );
}

#[tokio_shared_rt::test(shared)]
async fn clear_redis_recreates_post_content_index() -> Result<()> {
    StackManager::setup(&StackConfig::default())
        .await
        .map_err(|e| anyhow::anyhow!("could not initialise the stack: {e:?}"))?;

    // Same derivation as setup_cache, so a rename of PostDetails moves both sides.
    let prefix = format!("{}:", PostDetails::prefix().await);

    // Connector init already applied the schema before registering the pool.
    let before = post_content_index_info().await?;
    assert_post_content_schema(&before, &prefix, "after stack setup");

    // FLUSHDB destroys the index together with the keys; clear_redis must bring
    // the schema back on its own, without a migration run.
    clear_redis().await?;

    let after = post_content_index_info()
        .await
        .context("post content index must survive clear_redis()")?;
    assert_post_content_schema(&after, &prefix, "after clear_redis");

    // Restore the mock cache from the graph so the flush is not observable
    // by whatever runs after this test.
    reindex::sync().await;
    assert!(
        db_size().await? > 0,
        "reindex::sync should have repopulated Redis from the graph"
    );

    Ok(())
}
