use async_trait::async_trait;
use std::time::Instant;
use tracing::{info, warn};

use crate::migrations::manager::Migration;
use nexus_common::{
    db::{fetch_all_rows_from_graph, queries},
    models::post::{collection_item_keys, sync_collected_edges, PostDetails},
    types::DynError,
};
use pubky_app_specs::PubkyAppPostKind;

const PROGRESS_LOG_EVERY: u64 = 500;

pub struct CollectedEdgesBackfill1789344000;

#[async_trait]
impl Migration for CollectedEdgesBackfill1789344000 {
    fn id(&self) -> &'static str {
        "CollectedEdgesBackfill1789344000"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        // Materializes COLLECTED edges for collections indexed before the
        // watcher wrote them. Only the keys are snapshotted; each envelope is
        // read from the graph right before its reconcile, and the reconcile is
        // a no-op if the content moved on, so a concurrent edit is never
        // overwritten. Each sync also invalidates the items' cached counts and
        // is idempotent, so a failed run is simply re-run.
        let rows = fetch_all_rows_from_graph(queries::get::get_collection_posts()).await?;

        let started = Instant::now();
        let mut processed: u64 = 0;
        for row in rows {
            let author_id: String = row.get("author_id")?;
            let post_id: String = row.get("post_id")?;
            // Skip what stopped being a collection since the key snapshot; the
            // content guard alone would pass a kind-only edit.
            let fresh = PostDetails::get_from_graph(&author_id, &post_id)
                .await?
                .filter(|(details, _)| details.kind == PubkyAppPostKind::Collection);
            let Some((details, _)) = fresh else {
                continue;
            };
            let items = match collection_item_keys(&details.content) {
                Ok(items) => items,
                Err(e) => {
                    warn!("Collection {author_id}:{post_id} envelope malformed, skipped: {e}");
                    continue;
                }
            };
            sync_collected_edges(&author_id, &post_id, &items, Some(&details.content)).await?;

            processed += 1;
            if processed.is_multiple_of(PROGRESS_LOG_EVERY) {
                info!(
                    processed,
                    elapsed_secs = format!("{:.1}", started.elapsed().as_secs_f64()),
                    "CollectedEdgesBackfill progress"
                );
            }
        }

        info!(
            processed,
            elapsed_secs = format!("{:.1}", started.elapsed().as_secs_f64()),
            "CollectedEdgesBackfill completed"
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
