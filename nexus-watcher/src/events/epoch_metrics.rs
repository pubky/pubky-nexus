//! The operator-visible signal for events skipped by epoch: one counter labelled by `epoch`,
//! `resource` and `reason` only, never by user or object id. `reason` tells the two skips apart:
//! `not_indexed` for an epoch the parse step does not index, `gated` for an event of an epoch its
//! user is not read through. v0 skips at parse (last-read, feed, blob, mute, another app's tag
//! folder) are uncounted, so the signal stays about epochs.

use super::classify::SkipReason;
use super::target::EventTarget;
use crate::METER_NAME;
use nexus_common::models::user::Epoch;
use opentelemetry::metrics::Counter;
use opentelemetry::{global, KeyValue};
use std::sync::LazyLock;
use tracing::{debug, warn};

/// Counter for events skipped before any fetch because of their spec epoch.
static SKIPPED: LazyLock<Counter<u64>> = LazyLock::new(|| {
    global::meter(METER_NAME)
        .u64_counter("watcher.epoch.skipped")
        .with_description("Events skipped before any fetch because of their spec epoch")
        .build()
});

const UNKNOWN: &str = "unknown";

/// `reason` of a skip at parse: an epoch, or a resource of one, Nexus does not index.
const NOT_INDEXED: &str = "not_indexed";
/// `reason` of a skip by the gate: the event's epoch is not the one its user is read through.
const GATED: &str = "gated";

/// Counts and logs a skip the parse step decided for `uri`.
pub(super) fn record_skip(reason: &SkipReason, uri: &str) {
    if let Some((epoch, resource)) = skip_labels(reason, uri) {
        SKIPPED.add(1, &labels(epoch, resource, NOT_INDEXED));
    }
}

/// Counts and logs an event at `uri` the gate ignored, its user being at `user_epoch`.
pub(super) fn record_gated(target: &EventTarget, uri: &str, user_epoch: Epoch) {
    let epoch = target.epoch().as_str();
    let resource = target.resource_name();
    debug!(
        %uri,
        event_epoch = %epoch,
        user_epoch = %user_epoch.as_str(),
        "Ignored event: epochs do not match"
    );
    SKIPPED.add(1, &labels(epoch, &resource, GATED));
}

/// The full label set of one skip.
fn labels(epoch: &str, resource: &str, reason: &'static str) -> [KeyValue; 3] {
    [
        KeyValue::new("epoch", epoch.to_string()),
        KeyValue::new("resource", resource.to_string()),
        KeyValue::new("reason", reason),
    ]
}

/// The `(epoch, resource)` labels of a skip, logging it on the way; `None` for a v0 skip.
fn skip_labels<'a>(reason: &'a SkipReason, uri: &str) -> Option<(&'a str, &'a str)> {
    match reason {
        SkipReason::LegacyNotIndexed { .. } => None,
        SkipReason::AppFolderTag { app } => {
            debug!(%uri, %app, "Skipped tag in another app's folder");
            None
        }
        SkipReason::EpochNotIndexed { epoch, resource } => {
            debug!(%uri, %epoch, %resource, "Skipped event of an epoch Nexus does not index yet");
            Some((epoch_label(epoch), resource))
        }
        SkipReason::UnsupportedEpoch { version } => {
            warn!(%uri, %version, "Skipped event of a spec epoch this build does not speak");
            Some((epoch_label(version), UNKNOWN))
        }
    }
}

/// The `epoch` label of an untrusted path segment: the canonical spellings `v1` to `v255`
/// pass through, anything else becomes `unknown`, so the label set stays bounded.
fn epoch_label(version: &str) -> &str {
    let canonical = version
        .strip_prefix('v')
        .and_then(|n| {
            n.parse::<u8>()
                .ok()
                .filter(|v| *v != 0 && v.to_string() == n)
        })
        .is_some();
    if canonical {
        version
    } else {
        UNKNOWN
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pubky_social_specs::Resource;

    const URI: &str = "pubky://operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo/pub/social/v1/posts/0032SSN7Q4EVG";

    #[test]
    fn canonical_epochs_are_their_own_label() {
        for version in ["v1", "v2", "v10", "v255"] {
            assert_eq!(epoch_label(version), version);
        }
    }

    #[test]
    fn non_canonical_and_non_numeric_epochs_are_unknown() {
        for version in [
            "v0", "v01", "v256", "v1.0", "v", "v+1", "v-1", "V1", "1", "vx", "",
        ] {
            assert_eq!(epoch_label(version), UNKNOWN, "{version:?}");
        }
    }

    #[test]
    fn a_v1_skip_is_labelled_by_its_epoch_and_resource() {
        let reason = SkipReason::EpochNotIndexed {
            epoch: "v1".to_string(),
            resource: "posts".to_string(),
        };
        assert_eq!(skip_labels(&reason, URI), Some(("v1", "posts")));
    }

    #[test]
    fn an_unsupported_epoch_is_labelled_by_its_version_with_resource_unknown() {
        for (version, epoch) in [("v2", "v2"), ("v0", UNKNOWN), ("v256", UNKNOWN)] {
            let reason = SkipReason::UnsupportedEpoch {
                version: version.to_string(),
            };
            assert_eq!(
                skip_labels(&reason, URI),
                Some((epoch, UNKNOWN)),
                "{version}"
            );
        }
    }

    /// An unclassifiable v1 path carries the v1 parser's `unknown`; it must land on the same
    /// label as an unsupported epoch's resource.
    #[test]
    fn the_unknown_label_is_the_v1_parsers_spelling() {
        assert_eq!(Resource::Unknown.to_string(), UNKNOWN);
    }

    /// The gate's log names both epochs, so an ignored event can be explained.
    #[test]
    #[tracing_test::traced_test]
    fn a_gated_event_logs_that_the_epochs_do_not_match() {
        let line = "PUT pubky://operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo/pub/pubky.app/posts/0032SSN7Q4EVG";
        let event = match crate::events::Event::parse_event(line) {
            Ok(crate::events::ParseResult::Parsed(event)) => event,
            other => panic!("expected Parsed, got {other:?}"),
        };

        record_gated(&event.target, &event.uri, Epoch::V1);

        assert!(logs_contain("epochs do not match"));
        assert!(logs_contain("event_epoch=v0"));
        assert!(logs_contain("user_epoch=v1"));
    }

    #[test]
    fn labels_are_epoch_resource_and_reason_only() {
        let pairs =
            labels("v0", "posts", GATED).map(|kv| (kv.key.to_string(), kv.value.to_string()));
        let expected = [("epoch", "v0"), ("resource", "posts"), ("reason", "gated")]
            .map(|(k, v)| (k.to_string(), v.to_string()));
        assert_eq!(pairs, expected);
    }

    #[test]
    fn a_v0_skip_has_no_labels() {
        let reason = SkipReason::LegacyNotIndexed {
            resource: "mutes".to_string(),
        };
        assert_eq!(skip_labels(&reason, URI), None);
    }
}
