//! The one decision point after routing: index, skip, reject or invalid.

use super::route::Routed;
use super::target::EventTarget;
use super::translation::legacy_v0;
use pubky_social_specs::epoch_segment;

/// Why an event is skipped before any fetch: no GET, no retry entry, no stored event line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// A `pubky.app` resource Nexus never indexed: last-read, feed, blob or mute.
    LegacyNotIndexed { resource: String },
    /// A resource of a spec epoch this build recognises but does not index yet; `resource` is
    /// `unknown` for a path that epoch's parser cannot classify.
    EpochNotIndexed { epoch: String, resource: String },
    /// A `social/vN` epoch the pinned spec crate does not speak.
    UnsupportedEpoch { version: String },
}

/// What the watcher does with a routed address. Reasons name no URI; the caller has it.
#[derive(Debug)]
pub enum Classified {
    Index(EventTarget),
    Skip(SkipReason),
    /// Not an address Nexus recognises.
    Unrecognized(String),
    /// An address under a namespace Nexus indexes that names no resource.
    Invalid(String),
}

/// Decides index, skip, reject or invalid for a route; the v0 arms are in [`legacy_v0`].
pub fn classify(routed: Routed) -> Classified {
    match routed {
        Routed::Legacy(parsed) => legacy_v0::classify(parsed),
        Routed::Social(parsed) => Classified::Skip(SkipReason::EpochNotIndexed {
            epoch: epoch_segment(),
            resource: parsed.resource.to_string(),
        }),
        Routed::UnsupportedEpoch { version } => {
            Classified::Skip(SkipReason::UnsupportedEpoch { version })
        }
        Routed::Malformed { reason } => Classified::Unrecognized(reason),
    }
}
