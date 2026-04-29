mod errors;

use crate::{
    db::{kv::RedisResult, RedisOps},
    universal_tag::homeserver_parsed_uri::HomeserverParsedUri,
};
use pubky::Event as StreamEvent;
use pubky_app_specs::Resource;
use serde::{Deserialize, Serialize};
use std::{fmt, path::PathBuf};
use tracing::{debug, error};

pub use errors::EventProcessorError;

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

/// Result of parsing an event line from a homeserver.
#[derive(Debug)]
pub enum ParseResult {
    /// Successfully parsed into a known, actionable event.
    Parsed(Event),
    /// Known resource type that Nexus does not handle (e.g. LastRead, Feed, Blob).
    Skipped,
    /// URI was not recognised by pubky-app-specs. This may be an app-specific
    /// path (e.g. `/pub/mapky/tags/...`) or a genuinely malformed URI.
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

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let upper_case_str = match self {
            EventType::Put => "PUT",
            EventType::Del => "DEL",
        };
        write!(f, "{upper_case_str}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub uri: String,
    pub event_type: EventType,
    pub parsed_uri: HomeserverParsedUri,
    pub files_path: PathBuf,
    event_line: String,
}

impl RedisOps for Event {}

impl AsRef<[String]> for Event {
    fn as_ref(&self) -> &[String] {
        std::slice::from_ref(&self.event_line)
    }
}

impl Event {
    /// Parse event from a line returned by the homeserver's `/events` endpoint.
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

        Self::build(event_type, uri, event_line, files_path)
    }

    /// Constructs a nexus [`Event`] directly from a [`StreamEvent`], avoiding
    /// the string round-trip through [`Self::parse_event`].
    pub fn from_stream_event(
        stream_event: &StreamEvent,
        files_path: PathBuf,
    ) -> Result<Option<Self>, EventProcessorError> {
        let event_type: EventType = stream_event.event_type.clone().into();

        let uri = stream_event.resource.to_pubky_url();
        debug!("New stream event: {event_type} {uri}");

        let event_line = format!("{event_type} {uri}");
        match Self::build(event_type, uri, event_line, files_path)? {
            ParseResult::Parsed(event) => Ok(Some(event)),
            ParseResult::Skipped | ParseResult::UnrecognizedUri { .. } => Ok(None),
        }
    }

    fn build(
        event_type: EventType,
        uri: String,
        event_line: String,
        files_path: PathBuf,
    ) -> Result<ParseResult, EventProcessorError> {
        // Validate and parse the URI using HomeserverParsedUri. This handles both
        // standard pubky-app-specs URIs and universal tag URIs from other apps.
        let parsed_uri = match HomeserverParsedUri::try_from(uri.as_str()) {
            Ok(parsed) => parsed,
            Err(e) => {
                return Ok(ParseResult::unrecognized_uri(
                    event_type,
                    uri,
                    e.to_string(),
                ))
            }
        };

        if let HomeserverParsedUri::AppSpec {
            user_id: _,
            resource,
        } = &parsed_uri
        {
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

    /// Stores event line in Redis as part of the events list.
    #[tracing::instrument(name = "event.index.write", skip_all)]
    pub async fn store_event(&self) -> RedisResult<()> {
        self.put_index_list(&["Events"]).await
    }

    pub async fn get_events_from_redis(
        cursor: Option<u64>,
        limit: usize,
    ) -> RedisResult<(Vec<String>, u64)> {
        let start = cursor.unwrap_or(0);
        // Clamp to usize::MAX: on 32-bit targets u64 can exceed usize; the LRANGE
        // would return empty results for such a large index either way.
        let start_u = usize::try_from(start).unwrap_or(usize::MAX);
        let result = Event::try_from_index_list(&["Events"], Some(start_u), Some(limit)).await;

        let events = match result {
            Ok(r) => r.unwrap_or_default(),
            Err(error) => {
                error!("IndexReadFailed: Failed to read from list due to Redis error: {error}");
                return Err(error);
            }
        };

        let next_cursor = start + events.len() as u64;

        Ok((events, next_cursor))
    }
}
