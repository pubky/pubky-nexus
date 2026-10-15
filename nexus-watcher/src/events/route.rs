//! Routing of a pubky address to the spec epoch whose parser judges it.
//!
//! The namespace segment (`pubky.app`, `social`, another app) picks the parser, and a path is
//! never handed to another epoch's parser as a fallback. That keeps every sloppy shape the
//! frozen v0 parser accepts away from the v1 canonicaliser (see the parse tests in `event.rs`).
//! Routing takes no skip decision, so a later step can apply it to addresses found inside
//! objects; [`super::classify`] decides what the watcher does with a route.

use pubky_social_specs::legacy_v0::{try_parse_pubky_path, ExtendedParsedUri};
use pubky_social_specs::{epoch_segment, ParsedUri, Resource, SOCIAL_NAMESPACE};

/// A pubky address classified by its namespace segment and parsed by that epoch's parser.
#[derive(Debug)]
pub enum Routed {
    /// A `pubky.app` path or another app's universal tag, parsed by the frozen v0 reader.
    Legacy(ExtendedParsedUri),
    /// A `/pub/social/v1/…` path parsed by the v1 parser, an unknown v1 resource included.
    Social(ParsedUri),
    /// A `/pub/social/vN/…` path of an epoch the pinned crate does not speak.
    UnsupportedEpoch { version: String },
    /// No namespace, a `social` path without an epoch segment, or a path its own namespace's
    /// parser rejects.
    Malformed { reason: String },
}

/// Routes `uri` by its namespace segment, taken from the frozen reader's lenient splitter.
pub fn route(uri: &str) -> Routed {
    let path = match try_parse_pubky_path(uri) {
        Ok(path) => path,
        Err(reason) => return Routed::Malformed { reason },
    };
    if path.app == SOCIAL_NAMESPACE {
        return route_social(uri, path.segments.first().map(String::as_str));
    }
    match ExtendedParsedUri::try_from(uri) {
        Ok(parsed) => Routed::Legacy(parsed),
        Err(reason) => Routed::Malformed { reason },
    }
}

/// Routes a `social` path; `epoch` is the segment after the namespace as the splitter read it.
///
/// The v1 parser judges the shape and names an epoch it does not speak, but it reads an
/// epoch-less path as an unknown v1 resource, so the epoch segment is checked here: the
/// `social` namespace is reserved for the spec, never a universal tag of an app named `social`.
fn route_social(uri: &str, epoch: Option<&str>) -> Routed {
    let parsed = match ParsedUri::try_from(uri) {
        Ok(parsed) => parsed,
        Err(reason) => return Routed::Malformed { reason },
    };
    if let Resource::UnsupportedVersion { version } = parsed.resource {
        return Routed::UnsupportedEpoch { version };
    }
    match epoch {
        Some(epoch) if epoch == epoch_segment() => Routed::Social(parsed),
        _ => Routed::Malformed {
            reason: "social path without an epoch segment".to_string(),
        },
    }
}
