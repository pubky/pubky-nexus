use async_trait::async_trait;
use tracing::info;

use crate::migrations::{manager::Migration, utils::delete_keys_by_pattern_where};
use nexus_common::types::DynError;

/// Every key the retired user details cache lived under: `User:Details:{user_id}`.
const SCAN_PATTERN: &str = "User:Details:*";
const SCAN_COUNT: usize = 1000;

/// Retires the `User:Details` JSON documents. User details are read from the
/// graph now, and nothing writes the documents any more, so this only
/// reclaims memory: run it after this change's API and watcher are deployed.
/// An older watcher would recreate a document on its next profile event, and
/// this single-stage migration is marked done once it runs. Safe to re-run.
///
/// # Deploy ordering and rollback
/// Deploy the API before the watcher: once the watcher stops writing the
/// documents, the previous API serves stale profiles from them (a user
/// tombstoned after the deploy shows their old profile again). Rolling back
/// past this change is unsupported once this migration has run: the previous
/// build's list reads treat a missing document as a deleted user, so a
/// rollback would empty the user streams and evict every user from the
/// most-followed and influencers rankings as pages are read. Nothing on the
/// old build rebuilds the documents short of `db mock`, which flushes Redis.
pub struct UserDetailsCacheRetire1792160000;

#[async_trait]
impl Migration for UserDetailsCacheRetire1792160000 {
    fn id(&self) -> &'static str {
        "UserDetailsCacheRetire1792160000"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        let deleted = delete_keys_by_pattern_where(SCAN_PATTERN, SCAN_COUNT, |_| true).await?;
        info!(
            deleted,
            "UserDetailsCacheRetire: deleted retired user details documents from Redis"
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
