use async_trait::async_trait;

use crate::migrations::{manager::Migration, utils::delete_keys_by_pattern};
use nexus_common::types::DynError;
use tracing::info;

/// Removes all `RetryManager:*` keys from Redis.
///
/// This migration cleans up the legacy `RetryManager` index on prod. The old data
/// cannot be migrated (schema mismatch), so all entries are dropped to free the
/// prefix for safe reuse without deserialization issues.
pub struct RemoveRetryManager1781173800;

#[async_trait]
impl Migration for RemoveRetryManager1781173800 {
    fn id(&self) -> &'static str {
        "RemoveRetryManager1781173800"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        // Remove all RetryManager:* keys from Redis (sorted set + state JSON entries)
        let deleted = delete_keys_by_pattern("RetryManager:*", 100).await?;
        info!(
            "RemoveRetryManager migration: deleted {} RetryManager keys from Redis",
            deleted
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
