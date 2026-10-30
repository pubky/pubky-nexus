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
