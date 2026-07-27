use crate::migrations::manager::Migration;
use async_trait::async_trait;
use futures::StreamExt;
use nexus_common::db::graph::Query;
use nexus_common::db::{get_neo4j_graph, RedisOps};
use nexus_common::models::user::UserDetails;
use nexus_common::types::DynError;
use tracing::info;

/// Migrate from the `[DELETED]` name sentinel to a boolean `deleted` property.
///
/// # What this does
/// - Sets `u.deleted = true` where `name = '[DELETED]'`, `false` everywhere else, so the
///   property is present on every `:User` (enables dropping `coalesce()` later).
/// - Invalidates cached tombstone JSON per page. Live users' pre-migration entries lack
///   the key and deserialize to `false`, which is already correct.
///
/// # Batching
/// Nothing indexes `:User(deleted)` or `:User(name)`, so the backfill scans once and lets
/// the server commit per batch via `IN TRANSACTIONS`, instead of re-scanning per batch.
/// Tombstone IDs use keyset pagination over the `uniqueUserId` index; `SKIP`/`LIMIT`
/// replans as a label scan with `Limit $limit + $skip`, re-reading every earlier row.
///
/// # Idempotency
/// Safe to re-run: the `SET` falsifies the `WHERE`. `is_multi_staged()` is `false`, so the
/// manager marks this Done after one backfill; re-running needs the id in
/// `migrations_backfill_ready`. The second disjunct
/// (`u.name = '[DELETED]' AND u.deleted = false`) re-catches users tombstoned by
/// pre-cutover code. `IN TRANSACTIONS` commits per batch, so a mid-run failure leaves the
/// backfill partial and not Done, as the old client loop did; re-running finishes it.
///
/// # Rollback caveat
/// New-code tombstones leave `name` empty, not `'[DELETED]'`. Rolling back to code that
/// filters by the sentinel name makes them appear live, with an empty profile.
///
/// # Deploy ordering
/// Hard cutover: stop every pre-cutover instance (nexus-watcher is the only tombstone
/// writer), run `nexusd db migration run`, then start the new binaries. The first two
/// steps must not overlap — old code tombstones via the sentinel name without touching
/// `deleted`, so one written after the backfill drains reads as LIVE permanently,
/// recoverable only by a re-run. Between the last two both readers are consistent: the
/// backfill leaves `name` intact.
pub struct UserDeletedFlag1780617600;

/// Tombstone IDs invalidated per keyset page.
const TOMBSTONE_PAGE_SIZE: usize = 10_000;

#[async_trait]
impl Migration for UserDeletedFlag1780617600 {
    fn id(&self) -> &'static str {
        "UserDeletedFlag1780617600"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        let graph = get_neo4j_graph()?;

        // MATCH stays outside the subquery so IN TRANSACTIONS batches the rows it feeds in;
        // the trailing count forces the batches to run. Drain predicate: see # Idempotency.
        let query = Query::new(
            "user_deleted_flag_backfill",
            "MATCH (u:User)
             WHERE u.deleted IS NULL OR (u.name = '[DELETED]' AND u.deleted = false)
             CALL (u) {
                 SET u.deleted = coalesce(u.name = '[DELETED]', false)
             } IN TRANSACTIONS OF 10000 ROWS
             RETURN count(u) AS processed",
        );

        let mut result = graph.execute(query).await?;
        let processed: i64 = match result.next().await {
            Some(Ok(row)) => row.get::<i64>("processed")?,
            Some(Err(e)) => return Err(e.into()),
            None => 0,
        };
        info!("UserDeletedFlag migration: {} users backfilled", processed);

        // Tombstones only: live users' missing key already deserializes to false.
        let mut cursor = String::new();
        let mut total_tombstoned: usize = 0;

        loop {
            let page = Query::new(
                "user_deleted_flag_tombstones",
                "MATCH (u:User)
                 WHERE u.deleted = true AND u.id > $cursor
                 RETURN u.id AS id
                 ORDER BY id
                 LIMIT $limit",
            )
            .param("cursor", cursor.clone())
            .param("limit", TOMBSTONE_PAGE_SIZE as i64);

            let mut rows = graph.execute(page).await?;
            let mut ids: Vec<String> = Vec::with_capacity(TOMBSTONE_PAGE_SIZE);
            while let Some(row) = rows.next().await {
                ids.push(row?.get::<String>("id")?);
            }

            // Empty page: past the last tombstone.
            let Some(last_id) = ids.last().cloned() else {
                break;
            };
            cursor = last_id;

            let owned: Vec<Vec<&str>> = ids.iter().map(|id| vec![id.as_str()]).collect();
            let key_parts_list: Vec<&[&str]> = owned.iter().map(|k| k.as_slice()).collect();
            UserDetails::remove_from_index_multiple_json(&key_parts_list).await?;

            total_tombstoned += ids.len();
            info!(
                "UserDeletedFlag migration: invalidated {} tombstone cache entries ({} total)",
                ids.len(),
                total_tombstoned
            );

            if ids.len() < TOMBSTONE_PAGE_SIZE {
                break;
            }
        }

        info!(
            "UserDeletedFlag migration: complete — {} users backfilled, {} tombstones marked and cache-invalidated",
            processed, total_tombstoned
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
