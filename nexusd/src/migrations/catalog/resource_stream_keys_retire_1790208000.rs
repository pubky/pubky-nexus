use async_trait::async_trait;
use tracing::info;

use crate::migrations::{manager::Migration, utils::delete_keys_by_pattern_where};
use nexus_common::types::DynError;

/// Every key the retired resource stream sorted sets lived under.
const SCAN_PATTERN: &str = "Sorted:Resources:*";
const SCAN_COUNT: usize = 1000;

/// Retires the eight resource stream sorted sets the watcher used to maintain
/// per resource tag put/del:
///
/// ```text
/// Sorted:Resources:Global:Timeline
/// Sorted:Resources:Global:TaggersCount
/// Sorted:Resources:App:{app}:Timeline
/// Sorted:Resources:App:{app}:TaggersCount
/// Sorted:Resources:Tag:{label}:Timeline
/// Sorted:Resources:Tag:{label}:TaggersCount
/// Sorted:Resources:App:{app}:Tag:{label}:Timeline
/// Sorted:Resources:App:{app}:Tag:{label}:TaggersCount
/// ```
///
/// The stream is served from the graph now and never reads these keys, so
/// this only reclaims memory; run it after this change's watcher and API are
/// deployed. An older watcher recreates the keys on its next resource tag and
/// an older API would serve them as complete pages, and this single-stage
/// migration is marked done once it runs, so it would not delete them again.
/// Safe to re-run.
///
/// `Sorted:Resources:Tag:{resource_id}` (the per-resource label scores that
/// `TagResource` still maintains) shares the prefix and is kept.
pub struct ResourceStreamKeysRetire1790208000;

/// True for exactly the eight retired key shapes above.
///
/// The shapes are told apart by their fixed segments, not by the app or label
/// in between (those may themselves contain `:`). A `TagResource` key ends
/// with a resource id, never with `:Timeline` or `:TaggersCount`.
pub(crate) fn is_retired_resource_stream_key(key: &str) -> bool {
    let Some(rest) = key.strip_prefix("Sorted:Resources:") else {
        return false;
    };
    if matches!(rest, "Global:Timeline" | "Global:TaggersCount") {
        return true;
    }
    // App:{app}:…:Timeline / Tag:{label}:…:TaggersCount, with a non-empty middle
    let Some(scoped) = rest
        .strip_prefix("App:")
        .or_else(|| rest.strip_prefix("Tag:"))
    else {
        return false;
    };
    let middle = scoped
        .strip_suffix(":Timeline")
        .or_else(|| scoped.strip_suffix(":TaggersCount"));
    matches!(middle, Some(middle) if !middle.is_empty())
}

#[async_trait]
impl Migration for ResourceStreamKeysRetire1790208000 {
    fn id(&self) -> &'static str {
        "ResourceStreamKeysRetire1790208000"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        let deleted =
            delete_keys_by_pattern_where(SCAN_PATTERN, SCAN_COUNT, is_retired_resource_stream_key)
                .await?;
        info!(
            deleted,
            "ResourceStreamKeysRetire: deleted retired resource stream sorted sets from Redis"
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

#[cfg(test)]
mod tests {
    use super::is_retired_resource_stream_key;

    #[test]
    fn matches_exactly_the_eight_retired_shapes() {
        for key in [
            "Sorted:Resources:Global:Timeline",
            "Sorted:Resources:Global:TaggersCount",
            "Sorted:Resources:App:mapky:Timeline",
            "Sorted:Resources:App:mapky:TaggersCount",
            "Sorted:Resources:Tag:bitcoin:Timeline",
            "Sorted:Resources:Tag:bitcoin:TaggersCount",
            "Sorted:Resources:App:mapky:Tag:bitcoin:Timeline",
            "Sorted:Resources:App:mapky:Tag:bitcoin:TaggersCount",
            // Labels and apps may carry colons; the fixed segments still decide.
            "Sorted:Resources:Tag:a:b:Timeline",
            "Sorted:Resources:App:x:y:Tag:a:b:TaggersCount",
        ] {
            assert!(
                is_retired_resource_stream_key(key),
                "{key} should be retired"
            );
        }
    }

    #[test]
    fn keeps_live_keys_under_the_shared_prefix() {
        for key in [
            // TagResource label scores, keyed by resource id
            "Sorted:Resources:Tag:450a72e3da164bfc3ac5f4056f9e5c7c",
            // A label that only looks like a stream key on its own
            "Sorted:Resources:Tag:Timeline",
            "Sorted:Resources:Tag:TaggersCount",
            "Sorted:Resources:Global",
            "Sorted:Resources:Other:Timeline",
            "Sorted:Posts:Global:Timeline",
            "Resource:Taggers:450a72e3da164bfc3ac5f4056f9e5c7c:bitcoin",
        ] {
            assert!(!is_retired_resource_stream_key(key), "{key} must be kept");
        }
    }
}
