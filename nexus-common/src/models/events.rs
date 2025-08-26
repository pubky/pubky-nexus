use crate::db::RedisOps;
use crate::types::DynError;
use pubky_app_specs::{ParsedUri, Resource};
use serde::{Deserialize, Serialize};
use std::{fmt, path::PathBuf};
use thiserror::Error;
use utoipa::ToSchema;

#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum EventError {
    /// The event could not be parsed from a line
    #[error("InvalidEventLine: {message}")]
    InvalidEventLine { message: String },
}

impl RedisOps for Event {}

// Look for the end pattern after the start index, or use the end of the string if not found
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
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
}

impl Event {
    // TODO: files_path is not used anywhere not sure why it is needed
    pub fn parse_event(line: &str, files_path: PathBuf) -> Result<Option<Self>, DynError> {
        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() != 2 {
            return Err(EventError::InvalidEventLine {
                message: format!("Malformed event line, {line}"),
            }
            .into());
        }

        let event_type = match parts[0] {
            "PUT" => EventType::Put,
            "DEL" => EventType::Del,
            other => {
                return Err(EventError::InvalidEventLine {
                    message: format!("Unknown event type: {other}"),
                }
                .into())
            }
        };

        // Validate and parse the URI using pubky-app-specs
        let uri = parts[1].to_string();
        let parsed_uri =
            ParsedUri::try_from(uri.as_str()).map_err(|e| EventError::InvalidEventLine {
                message: format!("Cannot parse event URI: {e}"),
            })?;

        match parsed_uri.resource {
            // Unknown resource
            Resource::Unknown => {
                return Err(EventError::InvalidEventLine {
                    message: format!("Unknown resource in URI: {uri}"),
                }
                .into())
            }
            // Known resources not handled by Nexus
            Resource::LastRead | Resource::Feed(_) | Resource::Blob(_) => return Ok(None),
            _ => (),
        };

        Ok(Some(Event {
            uri,
            event_type,
            parsed_uri,
            files_path,
        }))
    }
}
