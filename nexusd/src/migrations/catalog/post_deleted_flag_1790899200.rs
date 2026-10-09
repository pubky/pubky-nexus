use crate::migrations::manager::Migration;
use async_trait::async_trait;
use futures::StreamExt;
use nexus_common::db::graph::Query;
use nexus_common::db::{get_neo4j_graph, RedisOps};
use nexus_common::models::post::PostDetails;
use nexus_common::types::DynError;
use tracing::info;

/// Migrate post tombstones from the `[DELETED]` content sentinel to a boolean `deleted`
/// property, as `UserDeletedFlag1780617600` did for users.
///
/// # What this does
/// - Sets `p.deleted = true` and clears `p.content` where `content = '[DELETED]'`, converging
///   old tombstones on the shape `post::del` writes now. Old ones already had `kind` forced to
///   Short and `attachments`/`lock` wiped by the pre-cutover `sync_put`, so `content` was the
///   only field that still differed. Clients that display the content no longer see two shapes.
/// - Live posts are left untouched, so `deleted IS NULL` stays a normal permanent state:
///   every reader treats absent as live, `get_post_by_id` via `COALESCE(p.deleted, false)`
///   and the cached JSON via `#[serde(default)]`.
/// - Invalidates cached tombstone JSON per page. The next read re-caches the cleared content
///   and the flag from the graph, and dropping the document also drops `[DELETED]` from the
///   post content search index. Live posts' pre-migration entries lack the key and
///   deserialize to `false`, which is already correct.
///
/// # Batching
/// Nothing indexes `:Post(deleted)` or `:Post(content)`, so the backfill scans once and lets
/// the server commit per batch via `IN TRANSACTIONS`, instead of re-scanning per batch. The
/// whole run still counts as one transaction against `db.transaction.timeout`. Tombstone keys
/// use keyset pagination over the `uniquePostId` index; `SKIP`/`LIMIT` replans as a label
/// scan with `Limit $limit + $skip`, re-reading every earlier row.
///
/// # Why the content is authoritative here
/// `PubkyAppPost::validate` rejects `[DELETED]` as content, so no live post can reach that
/// shape through a post event, and every sentinel-content row is a tombstone written by the
/// pre-cutover `post::del`. The codebase otherwise treats the flag as the only signal: a
/// live post with that content written below the validation boundary
/// (`test_live_post_with_sentinel_content_is_not_tombstoned`) must stay live, and a re-run
/// would tombstone it and destroy the content irreversibly.
///
/// # Idempotency
/// Safe to re-run: clearing `content` falsifies the `WHERE`. `is_multi_staged()` is `false`,
/// so the manager marks this Done after one backfill; re-running needs the id in
/// `migrations_backfill_ready`. `IN TRANSACTIONS` commits per batch, so a mid-run failure
/// leaves the backfill partial and not Done; re-running finishes it. The cache pass selects
/// by the flag, not the sentinel, so a re-run also invalidates the entries of tombstones an
/// earlier partial run already flagged.
///
/// # Rollback caveat
/// After this runs, *no* tombstone carries `'[DELETED]'` any more. Rolling back to code that
/// detects deletion by the sentinel content makes every tombstone a live post with empty
/// content, not just the ones written post-cutover. Only the flag distinguishes them, so a
/// rollback needs the flag-aware readers.
///
/// # Deploy ordering
/// Hard cutover: stop every pre-cutover instance (nexus-watcher is the only tombstone
/// writer), run `nexusd db migration run`, then start the new binaries. The first two steps
/// must not overlap: old code tombstones via the sentinel content without touching `deleted`,
/// so one written after the backfill drains reads as LIVE permanently, recoverable only by a
/// re-run. The post-backfill verify catches such a write if it lands before the check, but it
/// is a guard, not a guarantee.
pub struct PostDeletedFlag1790899200;

/// Tombstone keys invalidated per keyset page.
const TOMBSTONE_PAGE_SIZE: usize = 10_000;

#[async_trait]
impl Migration for PostDeletedFlag1790899200 {
    fn id(&self) -> &'static str {
        "PostDeletedFlag1790899200"
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
            "post_deleted_flag_backfill",
            "MATCH (p:Post)
             WHERE p.content = '[DELETED]'
             CALL (p) {
                 SET p.deleted = true, p.content = ''
             } IN TRANSACTIONS OF 10000 ROWS
             RETURN count(p) AS processed",
        );

        let mut result = graph.execute(query).await?;
        let processed: i64 = match result.next().await {
            Some(Ok(row)) => row.get::<i64>("processed")?,
            Some(Err(e)) => return Err(e.into()),
            None => 0,
        };
        info!(
            "PostDeletedFlag migration: {} tombstones flagged and content cleared",
            processed
        );

        // Re-run the drain predicate: a surviving sentinel means either the batches did not
        // all commit, or a pre-cutover writer tombstoned a post after the scan (see
        // # Deploy ordering). Erroring here leaves the migration not Done, forcing a re-run.
        let verify = Query::new(
            "post_deleted_flag_verify",
            "MATCH (p:Post)
             WHERE p.content = '[DELETED]'
             RETURN count(p) AS remaining",
        );

        let mut result = graph.execute(verify).await?;
        let remaining: i64 = match result.next().await {
            Some(Ok(row)) => row.get::<i64>("remaining")?,
            Some(Err(e)) => return Err(e.into()),
            None => return Err("PostDeletedFlag migration: verify query returned no rows".into()),
        };
        if remaining != 0 {
            return Err(format!(
                "PostDeletedFlag migration: backfill did not drain — {remaining} posts still \
                 hold '[DELETED]'. Confirm every pre-cutover instance is stopped, then re-run \
                 the migration before starting the new binaries."
            )
            .into());
        }

        // Tombstones only: live posts' missing key already deserializes to false. This pass
        // is also what propagates the cleared content — cached JSON still holds '[DELETED]'.
        let mut cursor = String::new();
        let mut total_tombstoned: usize = 0;

        loop {
            // The page is cut on the indexed post id before the author join, so a post
            // without an author still advances the cursor.
            let page = Query::new(
                "post_deleted_flag_tombstones",
                "MATCH (p:Post)
                 WHERE p.deleted = true AND p.id > $cursor
                 WITH p ORDER BY p.id LIMIT $limit
                 OPTIONAL MATCH (author:User)-[:AUTHORED]->(p)
                 RETURN p.id AS post_id, author.id AS author_id
                 ORDER BY post_id",
            )
            .param("cursor", cursor.clone())
            .param("limit", TOMBSTONE_PAGE_SIZE as i64);

            let mut rows = graph.execute(page).await?;
            let mut post_ids: Vec<String> = Vec::with_capacity(TOMBSTONE_PAGE_SIZE);
            let mut keys: Vec<[String; 2]> = Vec::with_capacity(TOMBSTONE_PAGE_SIZE);
            while let Some(row) = rows.next().await {
                let row = row?;
                let post_id = row.get::<String>("post_id")?;
                // No author, no cache key to invalidate
                if let Some(author_id) = row.get::<Option<String>>("author_id")? {
                    keys.push([author_id, post_id.clone()]);
                }
                post_ids.push(post_id);
            }

            // Empty page: past the last tombstone.
            let Some(last_id) = post_ids.last().cloned() else {
                break;
            };
            cursor = last_id;

            let key_parts_list: Vec<[&str; 2]> = keys
                .iter()
                .map(|[author_id, post_id]| [author_id.as_str(), post_id.as_str()])
                .collect();
            let key_parts_list: Vec<&[&str]> =
                key_parts_list.iter().map(|k| k.as_slice()).collect();
            PostDetails::remove_from_index_multiple_json(&key_parts_list).await?;

            total_tombstoned += keys.len();
            info!(
                "PostDeletedFlag migration: invalidated {} tombstone cache entries ({} total)",
                keys.len(),
                total_tombstoned
            );

            if post_ids.len() < TOMBSTONE_PAGE_SIZE {
                break;
            }
        }

        info!(
            "PostDeletedFlag migration: complete — {} tombstones flagged and cleared, {} cache entries invalidated",
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

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_common::db::exec_single_row;
    use nexus_common::{StackConfig, StackManager};
    use pubky_app_specs::PubkyAppPostKind;

    const AUTHOR_ID: &str = "MigrationTest:PostDeletedFlag:Author";
    const TOMBSTONE_ID: &str = "MigrationTest:PostDeletedFlag:Tombstone";
    const LIVE_ID: &str = "MigrationTest:PostDeletedFlag:Live";

    async fn delete_fixture() -> Result<(), DynError> {
        exec_single_row(
            Query::new(
                "post_deleted_flag_test_cleanup",
                "MATCH (n) WHERE (n:User AND n.id = $author) OR (n:Post AND n.id IN $posts)
                 DETACH DELETE n",
            )
            .param("author", AUTHOR_ID)
            .param("posts", vec![TOMBSTONE_ID, LIVE_ID]),
        )
        .await?;
        PostDetails::remove_from_index_multiple_json(&[
            &[AUTHOR_ID, TOMBSTONE_ID],
            &[AUTHOR_ID, LIVE_ID],
        ])
        .await?;
        Ok(())
    }

    /// A pre-cutover tombstone is flagged, its content cleared and its cached
    /// JSON dropped, while a live post of the same author is left alone. A
    /// second run changes nothing.
    #[tokio_shared_rt::test(shared)]
    async fn test_backfill_flags_legacy_tombstones() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        delete_fixture().await?;

        // The shape the old `post::del` wrote: the sentinel and no flag
        exec_single_row(
            Query::new(
                "post_deleted_flag_test_seed",
                "CREATE (u:User {id: $author, name: 'migration test'})
                 CREATE (u)-[:AUTHORED]->(:Post {id: $tombstone, uri: $tombstone_uri, content: '[DELETED]', kind: 'short', attachments: [], indexed_at: 1})
                 CREATE (u)-[:AUTHORED]->(:Post {id: $live, uri: $live_uri, content: 'still here', kind: 'short', indexed_at: 1})",
            )
            .param("author", AUTHOR_ID)
            .param("tombstone", TOMBSTONE_ID)
            .param("live", LIVE_ID)
            .param(
                "tombstone_uri",
                format!("pubky://{AUTHOR_ID}/pub/pubky.app/posts/{TOMBSTONE_ID}"),
            )
            .param(
                "live_uri",
                format!("pubky://{AUTHOR_ID}/pub/pubky.app/posts/{LIVE_ID}"),
            ),
        )
        .await?;
        let cached = PostDetails {
            content: "[DELETED]".to_string(),
            id: TOMBSTONE_ID.to_string(),
            author: AUTHOR_ID.to_string(),
            kind: PubkyAppPostKind::Short,
            ..Default::default()
        };
        cached
            .put_index_json(&[AUTHOR_ID, TOMBSTONE_ID], None, None)
            .await?;

        for run in 1..=2 {
            PostDeletedFlag1790899200.backfill().await?;

            let (tombstone, _) = PostDetails::get_from_graph(AUTHOR_ID, TOMBSTONE_ID)
                .await?
                .expect("the tombstone node is kept");
            assert!(tombstone.deleted, "run {run}: tombstone must be flagged");
            assert_eq!(tombstone.content, "", "run {run}: content must be cleared");

            let (live, _) = PostDetails::get_from_graph(AUTHOR_ID, LIVE_ID)
                .await?
                .expect("the live post is kept");
            assert!(!live.deleted, "run {run}: live post must stay live");
            assert_eq!(live.content, "still here");

            assert!(
                PostDetails::get_from_index(AUTHOR_ID, TOMBSTONE_ID)
                    .await?
                    .is_none(),
                "run {run}: the cached '[DELETED]' JSON must be invalidated"
            );
        }

        delete_fixture().await?;
        Ok(())
    }
}
