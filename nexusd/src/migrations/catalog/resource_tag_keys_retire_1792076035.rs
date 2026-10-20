use async_trait::async_trait;
use tracing::info;

use crate::migrations::{manager::Migration, utils::delete_keys_by_pattern};
use nexus_common::types::DynError;

/// Key prefixes no resource code reads or writes: resource streams and resource
/// tags are served from the graph.
const RETIRED_PATTERNS: [&str; 2] = ["Sorted:Resources:*", "Resource:Taggers:*"];
const SCAN_COUNT: usize = 1000;

/// Deletes the Redis keys resource tags no longer use:
///
/// ```text
/// Sorted:Resources:Tag:{resource_id}             label scores
/// Resource:Taggers:{resource_id}:{label}         taggers of a label
/// Resource:Taggers:{resource_id}:{app}:{label}   taggers of a label from one app
/// ```
///
/// No key under either prefix is live, so both go whole. That also clears any
/// resource stream set an older watcher wrote after
/// `ResourceStreamKeysRetire1790208000` ran.
///
/// Run it after this change's watcher and API are deployed. An older watcher
/// writes the keys again on its next resource tag, and an older API refills
/// them on a cache miss; this single-stage migration is marked done once it
/// runs, so it would not delete them again. Safe to re-run.
pub struct ResourceTagKeysRetire1792076035;

#[async_trait]
impl Migration for ResourceTagKeysRetire1792076035 {
    fn id(&self) -> &'static str {
        "ResourceTagKeysRetire1792076035"
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
                deleted, "ResourceTagKeysRetire: deleted retired resource tag keys from Redis"
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
