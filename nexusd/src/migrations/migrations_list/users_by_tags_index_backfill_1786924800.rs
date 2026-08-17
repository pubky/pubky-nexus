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
        // Run after the new watcher is live. The graph snapshot only
        // enumerates candidate (user, label) pairs; every score is derived
        // atomically from the live taggers set, so events landing during the
        // backfill are never overwritten with snapshot state.
        UsersByTagSearch::backfill_from_graph()
            .await
            .map_err(Into::into)
    }

    async fn cutover(&self) -> Result<(), DynError> {
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), DynError> {
        Ok(())
    }
}
