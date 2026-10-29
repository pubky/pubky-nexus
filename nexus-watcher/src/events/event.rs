use super::classify::{classify, Classified, SkipReason};
use super::epoch_metrics::record_skip;
use super::route::route;
use super::target::EventTarget;
use crate::errors::EventProcessorError;
use crate::homeserver::HsStreamEvent;
use nexus_common::models::event::EventLine;
use serde::{Deserialize, Serialize};
use std::fmt;
use tracing::{debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EventType {
    Put,
    Del,
}

impl From<pubky::EventType> for EventType {
    fn from(value: pubky::EventType) -> Self {
        match value {
            pubky::EventType::Put { .. } => Self::Put,
            pubky::EventType::Delete => Self::Del,
        }
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let upper_case_str = match self {
            EventType::Put => "PUT",
            EventType::Del => "DEL",
        };
        write!(f, "{upper_case_str}")
    }
}

/// Result of parsing an event line from a homeserver.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum ParseResult {
    /// Successfully parsed into a known, actionable event.
    Parsed(Event),
    /// Skipped before any fetch; see [`SkipReason`].
    Skipped(SkipReason),
    /// Not an address Nexus recognises: its namespace's parser rejects it, or it is not the
    /// canonical address of the resource it parses to (`/pub/pubky.app/posts/ID/extra`).
    UnrecognizedUri {
        event_type: EventType,
        uri: String,
        reason: String,
    },
}

impl ParseResult {
    fn unrecognized_uri(event_type: EventType, uri: String, reason: String) -> Self {
        Self::UnrecognizedUri {
            event_type,
            uri,
            reason,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Event {
    /// Pubky resource URI from the homeserver event line.
    pub uri: String,

    /// Operation represented by the event, used to dispatch to PUT or DEL handlers.
    pub event_type: EventType,

    /// The user and resource that [`Self::uri`] names.
    pub target: EventTarget,

    /// Original event line as received from the homeserver.
    event_line: String,
}

impl Event {
    /// Parse event from a line returned by the homeserver's `/events` endpoint.
    pub fn parse_event(line: &str) -> Result<ParseResult, EventProcessorError> {
        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() != 2 {
            return Err(EventProcessorError::InvalidEventLine(format!(
                "Malformed event line, {line}"
            )));
        }

        let event_type = match parts[0] {
            "PUT" => Ok(EventType::Put),
            "DEL" => Ok(EventType::Del),
            other => Err(EventProcessorError::InvalidEventLine(format!(
                "Unknown event type: {other}"
            ))),
        }?;

        let uri = parts[1].to_string();
        let event_line = line.to_string();

        Self::parse_event_parts(event_type, uri, event_line)
    }

    /// Constructs a nexus [`Event`] directly from a [`HsStreamEvent`], avoiding
    /// the string round-trip through [`Self::parse_event`].
    pub fn from_stream_event(
        stream_event: &HsStreamEvent,
    ) -> Result<Option<Self>, EventProcessorError> {
        let event_type = stream_event.event_type.clone();

        let uri = stream_event.uri().to_string();
        debug!(%event_type, %uri, "New stream event");

        let event_line = format!("{event_type} {uri}");
        match Self::parse_event_parts(event_type, uri, event_line)? {
            ParseResult::Parsed(event) => Ok(Some(event)),
            ParseResult::Skipped(_) => Ok(None),
            ParseResult::UnrecognizedUri { reason, .. } => {
                warn!(%reason, "Unrecognized event URI");
                Ok(None)
            }
        }
    }

    /// The one parse step behind the feed line, the stream event and the retry re-parse, so
    /// the same line gets the same outcome from every source.
    fn parse_event_parts(
        event_type: EventType,
        uri: String,
        event_line: String,
    ) -> Result<ParseResult, EventProcessorError> {
        let target = match classify(route(&uri)) {
            Classified::Index(target) => target,
            Classified::Skip(reason) => {
                record_skip(&reason, &uri);
                return Ok(ParseResult::Skipped(reason));
            }
            Classified::Unrecognized(reason) => {
                return Ok(ParseResult::unrecognized_uri(event_type, uri, reason))
            }
            Classified::Invalid(reason) => {
                return Err(EventProcessorError::InvalidEventLine(format!(
                    "{reason}: {uri}"
                )))
            }
        };

        if !target.is_canonical(&uri) {
            return Ok(ParseResult::unrecognized_uri(
                event_type,
                uri,
                "non-canonical path".to_string(),
            ));
        }

        Ok(ParseResult::Parsed(Event {
            uri,
            event_type,
            target,
            event_line,
        }))
    }

    pub fn to_event_line(&self) -> EventLine {
        EventLine::new(self.event_line.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pubky_social_specs::epoch_segment;

    const USER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const POST_ID: &str = "0032SSN7Q4EVG";
    const TAG_ID: &str = "ABCDEFGHJK";
    /// A canonical v1 `HashId` (26 Crockford chars, dangling bits clear).
    const HASH_ID: &str = "8Z8CWH8NVYQY39ZEBFGKQWWEKG";

    fn uri(path: &str) -> String {
        format!("pubky://{USER}/pub/{path}")
    }

    /// Parses `PUT` and `DEL` of `uri`, bypassing the line splitter so a URI with an embedded
    /// space reaches the router.
    fn parse_both(uri: &str) -> [Result<ParseResult, EventProcessorError>; 2] {
        [EventType::Put, EventType::Del].map(|kind| {
            Event::parse_event_parts(kind.clone(), uri.to_string(), format!("{kind} {uri}"))
        })
    }

    fn assert_parsed(path: &str) {
        let uri = uri(path);
        for result in parse_both(&uri) {
            match result {
                Ok(ParseResult::Parsed(event)) => assert_eq!(event.uri, uri),
                other => panic!("{uri}: expected Parsed, got {other:?}"),
            }
        }
    }

    fn assert_skipped(path: &str, expected: SkipReason) {
        let uri = uri(path);
        for result in parse_both(&uri) {
            match result {
                Ok(ParseResult::Skipped(reason)) => assert_eq!(reason, expected, "{uri}"),
                other => panic!("{uri}: expected Skipped, got {other:?}"),
            }
        }
    }

    /// Asserts `UnrecognizedUri` for the full `uri` and returns the (shared) reason.
    fn assert_unrecognized(uri: &str) -> String {
        let [put, del] = parse_both(uri).map(|result| match result {
            Ok(ParseResult::UnrecognizedUri {
                uri: reported,
                reason,
                ..
            }) => {
                assert_eq!(reported, uri);
                reason
            }
            other => panic!("{uri}: expected UnrecognizedUri, got {other:?}"),
        });
        assert_eq!(put, del, "{uri}");
        put
    }

    fn assert_non_canonical(uri: &str) {
        assert_eq!(assert_unrecognized(uri), "non-canonical path", "{uri}");
    }

    fn assert_invalid(path: &str) {
        let uri = uri(path);
        for result in parse_both(&uri) {
            match result {
                Err(EventProcessorError::InvalidEventLine(message)) => {
                    assert_eq!(message, format!("Unknown resource in URI: {uri}"))
                }
                other => panic!("{uri}: expected InvalidEventLine, got {other:?}"),
            }
        }
    }

    fn legacy_not_indexed(resource: &str) -> SkipReason {
        SkipReason::LegacyNotIndexed {
            resource: resource.to_string(),
        }
    }

    fn v1_not_indexed(resource: &str) -> SkipReason {
        SkipReason::EpochNotIndexed {
            epoch: epoch_segment(),
            resource: resource.to_string(),
        }
    }

    // ---- routing rows -------------------------------------------------------------------

    #[test]
    fn pubky_app_resources_nexus_indexes_are_parsed() {
        for path in [
            "pubky.app/profile.json".to_string(),
            format!("pubky.app/posts/{POST_ID}"),
            format!("pubky.app/follows/{USER}"),
            format!("pubky.app/bookmarks/{TAG_ID}"),
            format!("pubky.app/tags/{TAG_ID}"),
            format!("pubky.app/files/{POST_ID}"),
        ] {
            assert_parsed(&path);
        }
    }

    /// Last-read, feed and blob were never indexed; mutes join them here, so a mute costs no GET.
    #[test]
    fn pubky_app_resources_nexus_never_indexed_are_skipped() {
        for (path, resource) in [
            ("pubky.app/last_read".to_string(), "last_read"),
            (format!("pubky.app/feeds/{TAG_ID}"), "feeds"),
            (format!("pubky.app/blobs/{POST_ID}"), "blobs"),
            (format!("pubky.app/mutes/{USER}"), "mutes"),
        ] {
            assert_skipped(&path, legacy_not_indexed(resource));
        }
    }

    #[test]
    fn pubky_app_unknown_resource_is_an_invalid_event_line() {
        assert_invalid("pubky.app/foo/bar");
        assert_invalid("pubky.app/profile.json/extra");
    }

    #[test]
    fn unsupported_social_v1_resources_are_skipped_with_their_resource_name() {
        for (path, resource) in [
            (format!("social/v1/posts/{POST_ID}"), "posts"),
            (format!("social/v1/files/{HASH_ID}.jpg"), "files"),
            (format!("social/v1/feeds/{HASH_ID}.json"), "feeds"),
            // A v1 path the pinned parser cannot classify is still a v1 skip, under `unknown`.
            ("social/v1/whatever".to_string(), "unknown"),
            (format!("social/v1/posts/{POST_ID}.json"), "unknown"),
            ("social/v1".to_string(), "unknown"),
        ] {
            assert_skipped(&path, v1_not_indexed(resource));
        }
    }

    #[test]
    fn supported_social_v1_resources_are_indexed() {
        for path in [
            "social/v1/profile.json".into(),
            format!("social/v1/posts/{POST_ID}/{POST_ID}.json"),
            format!("social/v1/tags/{HASH_ID}.json"),
            format!("social/v1/follows/{USER}.json"),
        ] {
            assert_parsed(&path);
        }
    }

    #[test]
    fn unsupported_social_epoch_is_skipped_with_its_version() {
        for version in ["v2", "v0", "v01", "v256", "v99999"] {
            assert_skipped(
                &format!("social/{version}/posts/{POST_ID}"),
                SkipReason::UnsupportedEpoch {
                    version: version.to_string(),
                },
            );
        }
    }

    /// The `social` namespace is reserved for the spec: without an epoch segment it is not a
    /// universal tag of an app named `social`.
    #[test]
    fn epochless_social_path_is_unrecognized() {
        for path in [
            format!("social/tags/{TAG_ID}"),
            format!("social/posts/{POST_ID}"),
            "social/profile.json".to_string(),
            "social".to_string(),
        ] {
            assert_eq!(
                assert_unrecognized(&uri(&path)),
                "social path without an epoch segment",
                "{path}"
            );
        }
    }

    #[test]
    fn another_apps_tag_is_parsed_and_its_other_paths_are_unrecognized() {
        assert_parsed(&format!("mapky/tags/{TAG_ID}"));
        assert_parsed(&format!("eventky.app/tags/{TAG_ID}"));
        assert_unrecognized(&uri(&format!("mapky/posts/{POST_ID}")));
        assert_unrecognized(&uri("mapky/profile.json"));
    }

    #[test]
    fn unsplittable_uris_are_unrecognized() {
        for uri in [
            "pubky:///pub/pubky.app/posts/x".to_string(),
            "https://example.com/pub/pubky.app/posts/x".to_string(),
            format!("pubky://{USER}"),
            format!("pubky://{USER}/priv/pubky.app/posts/{POST_ID}"),
            format!("pubky://{USER}/priv/social/v1/posts/{POST_ID}"),
            format!("pubky://not-a-pubky-id/pub/pubky.app/posts/{POST_ID}"),
        ] {
            assert_unrecognized(&uri);
        }
    }

    // ---- the nine sloppy v0 shapes stay on the v0 parser -------------------------------

    /// Every shape the v0 parser accepts and the v1 parser rejects keeps today's outcome: the
    /// route is chosen by namespace, so the v1 canonicaliser never sees a `pubky.app` path.
    #[test]
    fn sloppy_v0_shapes_keep_their_outcome() {
        // Rendered differently from the event URI: the canonical check rejects them.
        for path in [
            format!("pubky.app/posts/{POST_ID}/"),
            format!("pubky.app/posts/{POST_ID}?x=1"),
            format!("pubky.app/posts/{POST_ID}#frag"),
            format!("pubky.app/x/../posts/{POST_ID}"),
            format!("pubky.app/posts/{POST_ID}\tX"),
            format!("pubky.app/posts/{POST_ID} X"),
        ] {
            assert_non_canonical(&uri(&path));
        }
        assert_non_canonical(&format!("PUBKY://{USER}/pub/pubky.app/posts/{POST_ID}"));
        // A double slash reads as an empty id, which the v0 parser calls an unknown resource.
        assert_invalid(&format!("pubky.app/posts//{POST_ID}"));
        assert_invalid(&format!("pubky.app//posts/{POST_ID}"));
        // A percent escape is part of the id for the v0 parser and round-trips.
        assert_parsed(&format!("pubky.app/posts/{POST_ID}%20"));
    }

    /// The same sloppy shapes under `social/v1` are judged by the v1 rules instead.
    #[test]
    fn sloppy_social_v1_shapes_are_unrecognized() {
        for path in [
            format!("social/v1/posts/{POST_ID}/"),
            format!("social/v1/posts//{POST_ID}"),
            format!("social/v1/posts/{POST_ID}?x=1"),
            format!("social/v1/posts/{POST_ID}#frag"),
            format!("social/v1/posts/{POST_ID}%20"),
            format!("social/v1/x/../posts/{POST_ID}"),
            format!("social/v1/posts/{POST_ID}\tX"),
            format!("social/v1/posts/{POST_ID} X"),
        ] {
            assert_unrecognized(&uri(&path));
        }
        assert_unrecognized(&format!("PUBKY://{USER}/pub/social/v1/posts/{POST_ID}"));
    }

    // ---- the canonical check runs after the route decision ------------------------------

    #[test]
    fn alias_paths_are_non_canonical() {
        for path in [
            format!("pubky.app/posts/{POST_ID}/shadow"),
            format!("pubky.app/tags/{TAG_ID}/extra/more"),
            format!("mapky/tags/{TAG_ID}?x=1"),
            format!("mapky/tags/{TAG_ID}#frag"),
        ] {
            assert_non_canonical(&uri(&path));
        }
        assert_non_canonical(&format!("PUBKY://{USER}/pub/mapky/tags/{TAG_ID}"));
    }

    /// The line splitter runs before the router: a line with the wrong shape never reaches it.
    #[test]
    fn line_splitter_still_rejects_malformed_lines() {
        for line in [
            "PUT".to_string(),
            "PATCH pubky://x".to_string(),
            format!("PUT {} extra", uri("pubky.app/profile.json")),
        ] {
            assert!(
                matches!(
                    Event::parse_event(&line),
                    Err(EventProcessorError::InvalidEventLine(_))
                ),
                "{line}"
            );
        }
    }

    /// The target's accessors are total: a resource without an id reads as an empty id.
    #[test]
    fn target_accessors_name_the_user_and_resource() {
        let cases = [
            (format!("pubky.app/posts/{POST_ID}"), "posts", POST_ID),
            ("pubky.app/profile.json".to_string(), "profile.json", ""),
            (format!("pubky.app/follows/{USER}"), "follows", USER),
            (format!("mapky/tags/{TAG_ID}"), "tags", TAG_ID),
        ];
        for (path, name, id) in cases {
            let Ok(ParseResult::Parsed(event)) = Event::parse_event(&format!("PUT {}", uri(&path)))
            else {
                panic!("{path}: expected Parsed");
            };
            assert_eq!(event.target.user_id().to_string(), USER, "{path}");
            assert_eq!(event.target.resource_name(), name, "{path}");
            assert_eq!(event.target.resource_id(), id, "{path}");
        }
    }
}
