use nexus_common::models::event::{EventProcessorError, RawEvent};
use nexus_common::universal_tag::homeserver_parsed_uri::HomeserverParsedUri;
use pubky::Event as StreamEvent;
use pubky_app_specs::Resource;
use serde::{Deserialize, Serialize};
use std::{fmt, path::PathBuf};
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

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum ParseResult {
    Parsed(Event),
    Skipped,
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
    pub uri: String,
    pub event_type: EventType,
    pub parsed_uri: HomeserverParsedUri,
    pub files_path: PathBuf,
    event_line: String,
}

impl Event {
    pub fn parse_event(
        line: &str,
        files_path: PathBuf,
    ) -> Result<ParseResult, EventProcessorError> {
        debug!("New event: {}", line);
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

        Self::parse_event_parts(event_type, uri, event_line, files_path)
    }

    /// Constructs an [`Event`] directly from a [`StreamEvent`], avoiding
    /// the string round-trip through [`Self::parse_event`].
    pub fn from_stream_event(
        stream_event: &StreamEvent,
        files_path: PathBuf,
    ) -> Result<Option<Self>, EventProcessorError> {
        let event_type: EventType = stream_event.event_type.clone().into();

        let uri = stream_event.resource.to_pubky_url();
        debug!("New stream event: {event_type} {uri}");

        let event_line = format!("{event_type} {uri}");
        match Self::parse_event_parts(event_type, uri, event_line, files_path)? {
            ParseResult::Parsed(event) => Ok(Some(event)),
            ParseResult::Skipped => Ok(None),
            ParseResult::UnrecognizedUri { reason, .. } => {
                warn!("Unrecognized event URI: {reason}");
                Ok(None)
            }
        }
    }

    fn parse_event_parts(
        event_type: EventType,
        uri: String,
        event_line: String,
        files_path: PathBuf,
    ) -> Result<ParseResult, EventProcessorError> {
        let parsed_uri = match HomeserverParsedUri::try_from(uri.as_str()) {
            Ok(parsed) => parsed,
            Err(e) => return Ok(ParseResult::unrecognized_uri(event_type, uri, e)),
        };

        if let HomeserverParsedUri::AppSpec { resource, .. } = &parsed_uri {
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

        Ok(ParseResult::Parsed(Event {
            uri,
            event_type,
            parsed_uri,
            files_path,
            event_line,
        }))
    }

    pub fn to_raw(&self) -> RawEvent {
        RawEvent(self.event_line.clone())
    }
}
