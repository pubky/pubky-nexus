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
/// - Sets `u.deleted = true` where `name = '[DELETED]'`. Live users are left untouched, so
///   `deleted IS NULL` stays a normal permanent state: every reader treats absent as live,
///   Cypher via `NOT coalesce(u.deleted, false)` and Rust via `deserialize_user_deleted`.
///   The filters keep their `coalesce()` — dropping it would make a missing property read
///   as deleted, which silently drops the user from `get_active_users_by_homeserver` and
///   stops the watcher indexing them.
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
/// `migrations_backfill_ready`. `coalesce()` in the predicate makes it match both an
/// unmigrated tombstone (no property at all) and one written by pre-cutover code after the
/// scan. `IN TRANSACTIONS` commits per batch, so a mid-run failure leaves the
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
/// recoverable only by a re-run. The post-backfill verify catches such a write if it lands
/// before the check, but it is a guard, not a guarantee. Between the last two both readers
/// are consistent: the backfill leaves `name` intact.
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
             WHERE u.name = '[DELETED]' AND coalesce(u.deleted, false) = false
             CALL (u) {
                 SET u.deleted = true
             } IN TRANSACTIONS OF 10000 ROWS
             RETURN count(u) AS processed",
        );

        let mut result = graph.execute(query).await?;
        let processed: i64 = match result.next().await {
            Some(Ok(row)) => row.get::<i64>("processed")?,
            Some(Err(e)) => return Err(e.into()),
            None => 0,
        };
        info!(
            "UserDeletedFlag migration: {} tombstones flagged",
            processed
        );

        // Re-run the drain predicate: a leftover row means either the batches did not all
        // commit, or a pre-cutover writer tombstoned a user after the scan (see # Deploy
        // ordering). Erroring here leaves the migration not Done, so a re-run is required.
        let verify = Query::new(
            "user_deleted_flag_verify",
            "MATCH (u:User)
             WHERE u.name = '[DELETED]' AND coalesce(u.deleted, false) = false
             RETURN count(u) AS remaining",
        );

        let mut result = graph.execute(verify).await?;
        let remaining: i64 = match result.next().await {
            Some(Ok(row)) => row.get::<i64>("remaining")?,
            Some(Err(e)) => return Err(e.into()),
            None => return Err("UserDeletedFlag migration: verify query returned no rows".into()),
        };
        if remaining != 0 {
            return Err(format!(
                "UserDeletedFlag migration: backfill did not drain — {remaining} tombstones \
                 still unflagged. Confirm every pre-cutover instance is stopped, then re-run \
                 the migration before starting the new binaries."
            )
            .into());
        }

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
            "UserDeletedFlag migration: complete — {} tombstones flagged, {} tombstones marked and cache-invalidated",
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
