mod errors;

use crate::db::RedisOps;
use crate::types::DynError;
use pubky_app_specs::{ParsedUri, Resource};
use serde::{Deserialize, Serialize};
use std::{fmt, path::PathBuf};
use tracing::{debug, error};

pub use errors::EventProcessorError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EventType {
    Put,
    Del,
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
    pub parsed_uri: ParsedUri,
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
    /// Parse event based on event line returned by homeservers' /events endpoint.
    /// - line - event line string
    /// - files_path - path to the directory where files are stored on nexus
    pub fn parse_event(line: &str, files_path: PathBuf) -> Result<Option<Self>, DynError> {
        debug!("New event: {}", line);
        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() != 2 {
            return Err(EventProcessorError::InvalidEventLine {
                message: format!("Malformed event line, {line}"),
            }
            .into());
        }

        let event_type = match parts[0] {
            "PUT" => Ok(EventType::Put),
            "DEL" => Ok(EventType::Del),
            other => Err(EventProcessorError::InvalidEventLine {
                message: format!("Unknown event type: {other}"),
            }),
        }?;

        // Validate and parse the URI using pubky-app-specs
        let uri = parts[1].to_string();
        let parsed_uri = ParsedUri::try_from(uri.as_str()).map_err(|e| {
            EventProcessorError::InvalidEventLine {
                message: format!("Cannot parse event URI: {e}"),
            }
        })?;

        match parsed_uri.resource {
            // Unknown resource
            Resource::Unknown => {
                return Err(EventProcessorError::InvalidEventLine {
                    message: format!("Unknown resource in URI: {uri}"),
                }
                .into())
            }
            // Known resources not handled by Nexus
            Resource::LastRead | Resource::Feed(_) | Resource::Blob(_) => return Ok(None),
            _ => (),
        };

        let event_line = line.to_string();

        Ok(Some(Event {
            uri,
            event_type,
            parsed_uri,
            files_path,
            event_line,
        }))
    }

    /// Stores event line in Redis as part of the events list.
    pub async fn store_event(&self) -> Result<(), DynError> {
        self.put_index_list(&["Events"]).await?;

        Ok(())
    }

    pub async fn get_events_from_redis(
        cursor: Option<usize>,
        limit: usize,
    ) -> Result<(Vec<String>, usize), DynError> {
        let start = cursor.unwrap_or(0);
        let result = Event::try_from_index_list(&["Events"], Some(start), Some(limit)).await;

        let events = match result {
            Ok(r) => r.unwrap_or_default(),
            Err(error) => {
                error!("IndexReadFailed: Failed to read from list due to Redis error: {error}");
                return Err(error.into());
            }
        };

        let next_cursor = start + events.len();

        Ok((events, next_cursor))
    }
}
