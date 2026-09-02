use async_trait::async_trait;
use futures::TryStreamExt;
use std::time::Instant;
use tracing::info;

use crate::migrations::manager::Migration;
use nexus_common::{
    db::{get_neo4j_graph, queries},
    models::user::UsersByTagSearch,
    types::DynError,
};

const PROGRESS_LOG_EVERY: u64 = 500;

pub struct UsersByTagsIndexBackfill1786924800;

#[async_trait]
impl Migration for UsersByTagsIndexBackfill1786924800 {
    fn id(&self) -> &'static str {
        "UsersByTagsIndexBackfill1786924800"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        // Run after the new watcher is live. The graph only enumerates
        // candidate (user, label) pairs, streamed to keep memory flat; every
        // score is derived atomically from the live taggers set, so events
        // landing during the backfill are never overwritten with snapshot
        // state, and a failed run can simply be re-run.
        let graph = get_neo4j_graph()?;
        let mut rows = graph.execute(queries::get::get_user_tag_pairs()).await?;

        let started = Instant::now();
        let mut processed: u64 = 0;
        while let Some(row) = rows.try_next().await? {
            let user_id: String = row.get("user_id")?;
            let label: String = row.get("label")?;
            UsersByTagSearch::sync_index_score(&user_id, &label).await?;

            processed += 1;
            if processed.is_multiple_of(PROGRESS_LOG_EVERY) {
                let elapsed = started.elapsed().as_secs_f64();
                info!(
                    processed,
                    elapsed_secs = format!("{elapsed:.1}"),
                    pairs_per_sec = format!("{:.0}", processed as f64 / elapsed),
                    "UsersByTagsIndexBackfill progress"
                );
            }
        }

        info!(
            processed,
            elapsed_secs = format!("{:.1}", started.elapsed().as_secs_f64()),
            "UsersByTagsIndexBackfill completed"
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
