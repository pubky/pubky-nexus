use async_trait::async_trait;
use tracing::info;

use crate::migrations::{manager::Migration, utils::delete_keys_by_pattern};
use nexus_common::types::DynError;

/// User counts are served from the graph, and no code reads or writes their
/// cached copy.
const RETIRED_PATTERN: &str = "User:Counts:*";
const SCAN_COUNT: usize = 1000;

/// Deletes the user counts documents:
///
/// ```text
/// User:Counts:{user_id}   counts JSON
/// ```
///
/// Run it after this change's watcher is deployed. An older watcher writes the
/// document again on a profile event and keeps updating the ones that exist;
/// this single-stage migration is marked done once it runs, so it would not
/// delete them again. Safe to re-run.
pub struct UserCountsKeysRetire1792418266;

#[async_trait]
impl Migration for UserCountsKeysRetire1792418266 {
    fn id(&self) -> &'static str {
        "UserCountsKeysRetire1792418266"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        let deleted = delete_keys_by_pattern(RETIRED_PATTERN, SCAN_COUNT).await?;
        info!(
            pattern = RETIRED_PATTERN,
            deleted, "UserCountsKeysRetire: deleted the user counts documents from Redis"
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
