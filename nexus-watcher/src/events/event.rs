use crate::errors::EventProcessorError;
use nexus_common::models::event::EventLine;
use pubky::Event as StreamEvent;
use pubky_app_specs::{ExtendedParsedUri, Resource};
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
    /// Known resource type that Nexus does not handle (e.g. LastRead, Feed, Blob).
    Skipped,
    /// URI was not recognised by pubky-app-specs. This may be an app-specific
    /// path (e.g. `/pub/mapky/tags/...`), a genuinely malformed URI, or a path that parses but
    /// is not the canonical address of its resource (e.g. `/pub/pubky.app/posts/ID/extra`).
    /// Callers should attempt fallback handling and log `reason` if no handler claims it.
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

    /// Parsed representation of [`Self::uri`].
    pub parsed_uri: ExtendedParsedUri,

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

    /// Constructs a nexus [`Event`] directly from a [`StreamEvent`], avoiding
    /// the string round-trip through [`Self::parse_event`].
    pub fn from_stream_event(
        stream_event: &StreamEvent,
    ) -> Result<Option<Self>, EventProcessorError> {
        let event_type: EventType = stream_event.event_type.clone().into();

        let uri = stream_event.resource.to_pubky_url();
        debug!(%event_type, %uri, "New stream event");

        let event_line = format!("{event_type} {uri}");
        match Self::parse_event_parts(event_type, uri, event_line)? {
            ParseResult::Parsed(event) => Ok(Some(event)),
            ParseResult::Skipped => Ok(None),
            ParseResult::UnrecognizedUri { reason, .. } => {
                warn!(%reason, "Unrecognized event URI");
                Ok(None)
            }
        }
    }

    fn parse_event_parts(
        event_type: EventType,
        uri: String,
        event_line: String,
    ) -> Result<ParseResult, EventProcessorError> {
        // Validate and parse the URI using ExtendedParsedUri. This handles both
        // standard pubky-app-specs URIs and universal tag URIs from other apps.
        let parsed_uri = match ExtendedParsedUri::try_from(uri.as_str()) {
            Ok(parsed) => parsed,
            Err(e) => return Ok(ParseResult::unrecognized_uri(event_type, uri, e)),
        };

        if let ExtendedParsedUri::PubkyApp { resource, .. } = &parsed_uri {
            match resource {
                Resource::Unknown => {
                    return Err(EventProcessorError::InvalidEventLine(format!(
                        "Unknown resource in URI: {uri}"
                    )))
                }
                Resource::LastRead | Resource::Feed(_) | Resource::Blob(_) => {
                    return Ok(ParseResult::Skipped)
                }
                _ => (),
            }
        }

        // The parser ignores segments past the resource id, so `posts/ID/extra` reads as
        // `posts/ID`. Accept only the address the specs render for the parsed resource, so an
        // alias path can neither overwrite nor delete the resource it parses to.
        match parsed_uri.try_to_uri_str() {
            Ok(canonical) if canonical == uri => {}
            _ => {
                return Ok(ParseResult::unrecognized_uri(
                    event_type,
                    uri,
                    "non-canonical path".to_string(),
                ))
            }
        }

        Ok(ParseResult::Parsed(Event {
            uri,
            event_type,
            parsed_uri,
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

    const USER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const POST_ID: &str = "0032SSN7Q4EVG";
    const TAG_ID: &str = "ABCDEFGHJK";

    fn uri(path: &str) -> String {
        format!("pubky://{USER}/pub/{path}")
    }

    fn parse(line: String) -> ParseResult {
        Event::parse_event(&line).expect("event line parses")
    }

    fn assert_parsed(path: &str) {
        let uri = uri(path);
        for kind in ["PUT", "DEL"] {
            match parse(format!("{kind} {uri}")) {
                ParseResult::Parsed(event) => assert_eq!(event.uri, uri),
                other => panic!("{kind} {uri}: expected Parsed, got {other:?}"),
            }
        }
    }

    fn assert_non_canonical(path: &str) {
        let uri = uri(path);
        for kind in ["PUT", "DEL"] {
            match parse(format!("{kind} {uri}")) {
                ParseResult::UnrecognizedUri { reason, .. } => {
                    assert_eq!(reason, "non-canonical path", "{kind} {uri}")
                }
                other => panic!("{kind} {uri}: expected UnrecognizedUri, got {other:?}"),
            }
        }
    }

    #[test]
    fn canonical_addresses_are_parsed() {
        assert_parsed(&format!("pubky.app/posts/{POST_ID}"));
        assert_parsed("pubky.app/profile.json");
        assert_parsed(&format!("pubky.app/tags/{TAG_ID}"));
        assert_parsed(&format!("pubky.app/follows/{USER}"));
        assert_parsed(&format!("mapky/tags/{TAG_ID}"));
    }

    #[test]
    fn extra_segments_are_rejected() {
        assert_non_canonical(&format!("pubky.app/posts/{POST_ID}/shadow"));
        assert_non_canonical(&format!("pubky.app/posts/{POST_ID}/"));
        assert_non_canonical(&format!("pubky.app/tags/{TAG_ID}/extra/more"));
    }

    #[test]
    fn query_and_fragment_are_rejected() {
        assert_non_canonical(&format!("pubky.app/posts/{POST_ID}?x=1"));
        assert_non_canonical(&format!("pubky.app/posts/{POST_ID}#frag"));
        assert_non_canonical(&format!("mapky/tags/{TAG_ID}?x=1"));
        assert_non_canonical(&format!("mapky/tags/{TAG_ID}#frag"));
    }

    #[test]
    fn uppercase_scheme_is_rejected() {
        let line = format!("PUT PUBKY://{USER}/pub/pubky.app/posts/{POST_ID}");
        assert!(
            matches!(parse(line), ParseResult::UnrecognizedUri { reason, .. } if reason == "non-canonical path")
        );
    }

    /// Unknown and skipped resources are classified before the canonical check, as before it.
    #[test]
    fn unknown_and_skipped_resources_keep_their_outcome() {
        for path in ["pubky.app/foo/bar", "pubky.app/profile.json/extra"] {
            let result = Event::parse_event(&format!("PUT {}", uri(path)));
            assert!(
                matches!(result, Err(EventProcessorError::InvalidEventLine(_))),
                "{path}: {result:?}"
            );
        }
        assert!(matches!(
            parse(format!("PUT {}", uri("pubky.app/last_read"))),
            ParseResult::Skipped
        ));
    }
}
