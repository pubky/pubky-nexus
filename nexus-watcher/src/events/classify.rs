//! The one decision point after routing: index, skip, reject or invalid.

use super::route::Routed;
use super::target::EventTarget;
use pubky_social_specs::epoch_segment;
use pubky_social_specs::legacy_v0::{ExtendedParsedUri, Resource};

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

/// Decides index, skip, reject or invalid for a route.
pub fn classify(routed: Routed) -> Classified {
    match routed {
        Routed::Legacy(parsed) => classify_legacy(parsed),
        Routed::Social(parsed) => {
            use pubky_social_specs::{Resource as V1Resource, Visibility};
            if parsed.visibility == Visibility::Public
                && matches!(
                    parsed.resource,
                    V1Resource::User
                        | V1Resource::Post {
                            version: Some(_),
                            ..
                        }
                        | V1Resource::Follow(_)
                        | V1Resource::Tag(_)
                )
            {
                Classified::Index(EventTarget::Social(parsed))
            } else {
                Classified::Skip(SkipReason::EpochNotIndexed {
                    epoch: epoch_segment(),
                    resource: parsed.resource.to_string(),
                })
            }
        }
        Routed::UnsupportedEpoch { version } => {
            Classified::Skip(SkipReason::UnsupportedEpoch { version })
        }
        Routed::Malformed { reason } => Classified::Unrecognized(reason),
    }
}

/// Index, skip or reject a v0 address; the resources Nexus never indexed are skipped.
fn classify_legacy(parsed: ExtendedParsedUri) -> Classified {
    match parsed.resource() {
        Resource::User
        | Resource::Post(_)
        | Resource::Follow(_)
        | Resource::Bookmark(_)
        | Resource::Tag(_)
        | Resource::File(_) => Classified::Index(EventTarget::Legacy(parsed)),
        resource @ (Resource::LastRead
        | Resource::Feed(_)
        | Resource::Blob(_)
        | Resource::Mute(_)) => Classified::Skip(SkipReason::LegacyNotIndexed {
            resource: resource.to_string(),
        }),
        Resource::Unknown => Classified::Invalid("Unknown resource in URI".to_string()),
    }
}
