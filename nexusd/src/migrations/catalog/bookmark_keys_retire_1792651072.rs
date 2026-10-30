use async_trait::async_trait;
use tracing::info;

use crate::migrations::{manager::Migration, utils::delete_keys_by_pattern};
use nexus_common::types::DynError;

/// Bookmarks are served from the graph, and no code reads or writes these keys.
const RETIRED_PATTERNS: [&str; 2] = ["Bookmark:*", "Sorted:Bookmarks:User:*"];
const SCAN_COUNT: usize = 1000;

/// Deletes the bookmark copies:
///
/// ```text
/// Bookmark:{author_id}:{post_id}:{user_id}   bookmark JSON
/// Sorted:Bookmarks:User:{user_id}            the user's bookmarked posts
/// ```
///
/// Run it after this change's watcher and API are deployed. An older watcher
/// writes both keys again on a bookmark, and an older API refills them on a
/// read; this single-stage migration is marked done once it runs, so it would
/// not delete them again. Safe to re-run.
pub struct BookmarkKeysRetire1792651072;

#[async_trait]
impl Migration for BookmarkKeysRetire1792651072 {
    fn id(&self) -> &'static str {
        "BookmarkKeysRetire1792651072"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        for pattern in RETIRED_PATTERNS {
            let deleted = delete_keys_by_pattern(pattern, SCAN_COUNT).await?;
            info!(
                pattern,
                deleted, "BookmarkKeysRetire: deleted the bookmark copies from Redis"
            );
        }
        Ok(())
    }

    async fn cutover(&self) -> Result<(), DynError> {
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), DynError> {
        Ok(())
    }
}
