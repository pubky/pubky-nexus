use crate::migrations::manager::Migration;
use async_trait::async_trait;
use nexus_common::db::graph::Query;
use nexus_common::db::{
    fetch_all_rows_from_graph, fetch_key_from_graph, get_redis_conn, queries, RedisOps,
};
use nexus_common::models::user::{Epoch, UserDetails};
use nexus_common::types::DynError;
use redis::Script;
use tracing::info;

/// Mark every user indexed before epochs existed as `v0`, so they don't wait for their next
/// event to get one.
///
/// # What this does
/// 1. Graph: sets `epoch = 'v0'` on every `:User` without one, tombstones and stubs included,
///    then fails if any `:User` is still without one.
/// 2. Redis: sets `$.epoch` to `"v0"` in every cached `User:Details` document, one keyset
///    page of user ids at a time. A missing document is never created.
///
/// The cached JSON is rewritten in place, never deleted: the user streams and the recommended,
/// most-followed and influencers caches read `User:Details` with no graph fallback and treat a
/// missing document as a deleted user.
///
/// # Batching
/// Nothing indexes `:User(epoch)`, so the graph step scans once and lets the server commit per
/// batch via `IN TRANSACTIONS`, instead of re-scanning per batch. The Redis step pages ids over
/// the `:User(id)` index by keyset; `SKIP`/`LIMIT` would re-read every earlier row.
///
/// # Why `v0` is hard-coded
/// No user can be on any other epoch until the flip ships, so the Redis step needs no read of
/// the graph's value. **It must not be rerun once the flip has shipped**: it would reset the
/// flipped users' cached `User:Details` to `v0`, out of step with the graph.
///
/// # Idempotency
/// The graph step only matches users without an epoch and `JSON.SET` overwrites with `v0`, so
/// a failed run is safe to repeat. `is_multi_staged()` is `false`, so the manager marks this
/// Done after one backfill; running it again needs the id in `migrations_backfill_ready`.
///
/// # Deploy ordering
/// Run it after the binaries that give users an epoch are deployed, and before the flip ships,
/// with every watcher stopped: a pre-epoch watcher could create a user after the graph step,
/// and the verify only catches that if the user lands before the check.
pub struct UserEpochBackfill1791812997;

/// User ids rewritten in Redis per keyset page.
const USER_PAGE_SIZE: usize = 1_000;

/// Fails when a `:User` is still without an epoch, so a partial graph step can't pass unnoticed.
async fn verify_every_user_has_epoch() -> Result<(), DynError> {
    let verify = Query::new(
        "user_epoch_backfill_verify",
        "MATCH (u:User)
         WHERE u.epoch IS NULL
         RETURN count(u) AS remaining",
    );

    let remaining: i64 = fetch_key_from_graph(verify, "remaining")
        .await?
        .unwrap_or_default();
    if remaining != 0 {
        return Err(format!(
            "UserEpochBackfill migration: backfill did not drain — {remaining} users still have \
             no epoch. Confirm every pre-epoch watcher is stopped, then re-run the migration."
        )
        .into());
    }
    Ok(())
}

/// Sets `$.epoch` to `"v0"` in the cached `User:Details` document of each user that has one,
/// and returns how many were set. A missing document is skipped, never created; one that
/// isn't a JSON object is skipped too and not counted.
///
/// One Lua script covers the page, so a document can't be deleted between the existence
/// check and the write.
async fn set_cached_details_epoch_v0(user_ids: &[String]) -> Result<usize, DynError> {
    let script = Script::new(
        r#"
        local set = 0
        for _, key in ipairs(KEYS) do
            -- JSON.SET replies nil, not an error, when the document isn't an object
            if redis.call('EXISTS', key) == 1 and redis.call('JSON.SET', key, '$.epoch', ARGV[1]) then
                set = set + 1
            end
        end
        return set
    "#,
    );

    let prefix = UserDetails::prefix().await;
    let mut invocation = script.prepare_invoke();
    for user_id in user_ids {
        invocation.key(format!("{prefix}:{user_id}"));
    }
    invocation.arg(format!("\"{}\"", Epoch::V0.as_str()));

    let mut conn = get_redis_conn().await?;
    Ok(invocation.invoke_async(&mut conn).await?)
}

#[async_trait]
impl Migration for UserEpochBackfill1791812997 {
    fn id(&self) -> &'static str {
        "UserEpochBackfill1791812997"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        // MATCH stays outside the subquery so IN TRANSACTIONS batches the rows it feeds in;
        // the trailing count forces the batches to run.
        let query = Query::new(
            "user_epoch_backfill",
            "MATCH (u:User)
             WHERE u.epoch IS NULL
             CALL (u) {
                 SET u.epoch = 'v0'
             } IN TRANSACTIONS OF 10000 ROWS
             RETURN count(u) AS processed",
        );

        let processed: i64 = fetch_key_from_graph(query, "processed")
            .await?
            .unwrap_or_default();
        info!("UserEpochBackfill migration: {processed} users set to v0 in the graph");

        verify_every_user_has_epoch().await?;

        let mut after_id = String::new();
        let mut total_users: usize = 0;
        let mut total_rewritten: usize = 0;

        loop {
            // Only the ids are read: see # Why `v0` is hard-coded
            let query = queries::get::get_user_ids_page(&after_id, USER_PAGE_SIZE);
            let mut user_ids: Vec<String> = Vec::with_capacity(USER_PAGE_SIZE);
            for row in fetch_all_rows_from_graph(query).await? {
                user_ids.push(row.get("id")?);
            }

            // Only an empty page is the end: the query clamps its limit, so a short page
            // may not be.
            let Some(last_id) = user_ids.last().cloned() else {
                break;
            };
            after_id = last_id;

            let rewritten = set_cached_details_epoch_v0(&user_ids).await?;

            total_users += user_ids.len();
            total_rewritten += rewritten;
            info!(
                "UserEpochBackfill migration: walked {} users ({total_users} total), \
                 rewrote {rewritten} cached details ({total_rewritten} total)",
                user_ids.len()
            );
        }

        info!(
            "UserEpochBackfill migration: complete — {processed} users set to v0 in the graph, \
             {total_users} walked, {total_rewritten} cached details rewritten"
        );

        Ok(())
    }

    async fn cutover(&self) -> Result<(), DynError> {
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), DynError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::FutureExt;
    use nexus_common::db::exec_single_row;
    use nexus_common::{StackConfig, StackManager};
    use std::panic::AssertUnwindSafe;

    /// Users with a cached `User:Details` document.
    const CACHED_IDS: [&str; 2] = [
        "MigrationTest:UserEpochBackfill:Cached1",
        "MigrationTest:UserEpochBackfill:Cached2",
    ];
    /// Users without one.
    const UNCACHED_IDS: [&str; 2] = [
        "MigrationTest:UserEpochBackfill:Uncached1",
        "MigrationTest:UserEpochBackfill:Uncached2",
    ];
    /// A user left without an epoch for the verify to catch.
    const NO_EPOCH_ID: &str = "MigrationTest:UserEpochBackfill:NoEpoch";
    /// A user whose cached document isn't a JSON object.
    const SCALAR_ID: &str = "MigrationTest:UserEpochBackfill:Scalar";

    fn all_ids() -> Vec<&'static str> {
        CACHED_IDS.iter().chain(&UNCACHED_IDS).copied().collect()
    }

    fn details_key(user_id: &str) -> String {
        format!("User:Details:{user_id}")
    }

    /// A `User:Details` document as cached before epochs existed: no `epoch` field.
    fn pre_epoch_document(user_id: &str) -> serde_json::Value {
        serde_json::json!({
            "name": format!("{user_id} name"),
            "bio": "kept",
            "id": user_id,
            "links": null,
            "status": null,
            "image": null,
            "indexed_at": 1,
            "deleted": false,
        })
    }

    async fn delete_fixture(user_ids: &[&str]) -> Result<(), DynError> {
        exec_single_row(
            Query::new(
                "user_epoch_backfill_test_cleanup",
                "MATCH (u:User) WHERE u.id IN $ids DETACH DELETE u",
            )
            .param("ids", user_ids.to_vec()),
        )
        .await?;
        let keys: Vec<String> = user_ids.iter().map(|id| details_key(id)).collect();
        let mut conn = get_redis_conn().await?;
        redis::cmd("DEL")
            .arg(&keys)
            .query_async::<()>(&mut conn)
            .await?;
        Ok(())
    }

    /// Creates `:User` nodes in the shape written before epochs existed.
    async fn seed_pre_epoch_users(user_ids: &[&str]) -> Result<(), DynError> {
        exec_single_row(
            Query::new(
                "user_epoch_backfill_test_seed",
                "UNWIND $ids AS id
                 CREATE (:User {id: id, name: 'migration test', indexed_at: 1})",
            )
            .param("ids", user_ids.to_vec()),
        )
        .await?;
        Ok(())
    }

    async fn graph_epoch(user_id: &str) -> Result<Option<String>, DynError> {
        let query = Query::new(
            "user_epoch_backfill_test_epoch",
            "MATCH (u:User {id: $user_id}) RETURN u.epoch AS epoch",
        )
        .param("user_id", user_id);
        Ok(fetch_key_from_graph::<Option<String>>(query, "epoch")
            .await?
            .flatten())
    }

    async fn cached_document(user_id: &str) -> Result<Option<serde_json::Value>, DynError> {
        let mut conn = get_redis_conn().await?;
        let raw: Option<String> = redis::cmd("JSON.GET")
            .arg(details_key(user_id))
            .query_async(&mut conn)
            .await?;
        Ok(raw.map(|raw| serde_json::from_str(&raw)).transpose()?)
    }

    /// Runs the backfill twice and checks the seeded users after each run.
    async fn backfill_twice_and_check() -> Result<(), DynError> {
        for run in 1..=2 {
            UserEpochBackfill1791812997.backfill().await?;

            for user_id in all_ids() {
                assert_eq!(
                    graph_epoch(user_id).await?.as_deref(),
                    Some("v0"),
                    "run {run}: {user_id} graph epoch"
                );
            }
            for user_id in CACHED_IDS {
                let mut expected = pre_epoch_document(user_id);
                expected["epoch"] = serde_json::json!("v0");
                assert_eq!(
                    cached_document(user_id).await?,
                    Some(expected),
                    "run {run}: {user_id} cached details"
                );
            }
            for user_id in UNCACHED_IDS {
                assert_eq!(
                    cached_document(user_id).await?,
                    None,
                    "run {run}: {user_id} must not get a cached document"
                );
            }
        }
        Ok(())
    }

    /// Every seeded user gets `v0` in the graph; the cached documents get `$.epoch` and keep
    /// every other field, and no document is created. A second run changes nothing.
    ///
    /// The backfill also sets `v0` on the mock users without an epoch, which reads the same.
    #[tokio_shared_rt::test(shared)]
    async fn test_backfill_marks_every_user_v0() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        delete_fixture(&all_ids()).await?;

        seed_pre_epoch_users(&all_ids()).await?;
        let mut conn = get_redis_conn().await?;
        for user_id in CACHED_IDS {
            redis::cmd("JSON.SET")
                .arg(details_key(user_id))
                .arg("$")
                .arg(pre_epoch_document(user_id).to_string())
                .query_async::<()>(&mut conn)
                .await?;
        }

        let outcome = AssertUnwindSafe(backfill_twice_and_check())
            .catch_unwind()
            .await;

        delete_fixture(&all_ids()).await?;
        outcome.unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    }

    /// A cached document that isn't a JSON object is kept as it is and not counted.
    #[tokio_shared_rt::test(shared)]
    async fn test_rewrite_skips_a_non_object_document() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        delete_fixture(&[SCALAR_ID]).await?;

        let mut conn = get_redis_conn().await?;
        redis::cmd("JSON.SET")
            .arg(details_key(SCALAR_ID))
            .arg("$")
            .arg("\"not an object\"")
            .query_async::<()>(&mut conn)
            .await?;

        let result = set_cached_details_epoch_v0(&[SCALAR_ID.to_string()]).await;
        let stored = cached_document(SCALAR_ID).await;

        delete_fixture(&[SCALAR_ID]).await?;
        assert_eq!(result?, 0);
        assert_eq!(stored?, Some(serde_json::json!("not an object")));
        Ok(())
    }

    /// A `:User` still without an epoch after the graph step fails the migration.
    #[tokio_shared_rt::test(shared)]
    async fn test_verify_fails_on_a_user_without_epoch() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        delete_fixture(&[NO_EPOCH_ID]).await?;

        seed_pre_epoch_users(&[NO_EPOCH_ID]).await?;
        let result = verify_every_user_has_epoch().await;

        delete_fixture(&[NO_EPOCH_ID]).await?;
        let err = result.expect_err("a user without an epoch must fail the verify");
        assert!(err.to_string().contains("still have no epoch"), "{err}");
        Ok(())
    }
}
