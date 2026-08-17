use async_trait::async_trait;

use crate::migrations::manager::Migration;
use nexus_common::{models::user::UsersByTagSearch, types::DynError};

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
        // Build the per-label users-by-tag sorted sets from a graph snapshot,
        // run after the new watcher is live. Puts overwritten mid-backfill are
        // safe; a delete landing between the graph read and the ZADD can be
        // resurrected and stays until the next event for that (user, label).
        // Same accepted race class as PostsByTagSearch::reindex.
        UsersByTagSearch::reindex().await.map_err(Into::into)
    }

    async fn cutover(&self) -> Result<(), DynError> {
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), DynError> {
        Ok(())
    }
}
