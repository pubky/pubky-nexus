use async_trait::async_trait;
use tracing::info;

use crate::migrations::{manager::Migration, utils::delete_keys_by_pattern};
use nexus_common::types::DynError;

/// The JSON copies of file details: `File:Details:{owner_id}:{file_id}`.
const RETIRED_PATTERN: &str = "File:Details:*";
const SCAN_COUNT: usize = 1000;

/// Deletes the Redis keys file details no longer use.
///
/// File details are read from the graph on every call; the watcher no longer
/// writes the JSON copy on a file put and no read refills it, so the keys only
/// hold memory. Run it after this change's watcher and API are deployed. An
/// older watcher writes the key again on its next file put, and an older API
/// refills it on a cache miss; this single-stage migration is marked done once
/// it runs, so it would not delete them again. Safe to re-run.
pub struct FileDetailsKeysRetire1792145757;

#[async_trait]
impl Migration for FileDetailsKeysRetire1792145757 {
    fn id(&self) -> &'static str {
        "FileDetailsKeysRetire1792145757"
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
            deleted,
            "FileDetailsKeysRetire: deleted retired file details keys from Redis"
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
