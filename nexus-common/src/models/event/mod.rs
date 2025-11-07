mod errors;

use crate::db::{kv::SortOrder, RedisOps};
use crate::types::DynError;
use pubky_app_specs::{ParsedUri, Resource};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
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
}

impl RedisOps for Event {}

impl Event {
    /// Parse event based on event line retured by homeservers' /events endpoint.
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
            "PUT" => EventType::Put,
            "DEL" => EventType::Del,
            other => {
                return Err(EventProcessorError::InvalidEventLine {
                    message: format!("Unknown event type: {other}"),
                }
                .into())
            }
        };

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

        Ok(Some(Event {
            uri,
            event_type,
            parsed_uri,
            files_path,
        }))
    }

    /// Stores event at redis as a member of stored set.
    /// Sorting is done by synchronization timestamp.
    pub async fn store_event(&self) -> Result<(), DynError> {
        // We use sync timestamp instead of homeserver cursor (creation timestamp)
        // because we want to ensure that events are processed in the order they
        // were received and they can not be inserted in between already processed events.
        //
        // At the same time the timestamp is the same as it is used in homeserver in order
        // to simplify migration from homeserver /events endpoint to nexus /events endpoint
        // aiming to make them mutually replacable from the consumer standpoint.
        let ts_ms = SystemTime::now().duration_since(UNIX_EPOCH)?.as_micros() as f64;
        let line = format!("{} {}", self.event_type, self.uri);

        let elements = vec![(ts_ms, line.as_str())];
        debug!("Storing event line: {ts_ms} {line}");

        // While events can be removed basically right after they were exposed to consumer, it
        // is better to have certain retention policy to avoid accidental data loss.
        // Therefore we have 180 days retention policy "just in case".
        let expiration = 180 * 24 * 60 * 60;

        Event::put_index_sorted_set(&["Events"], &elements, None, Some(expiration)).await
    }

    pub async fn get_events_from_redis(
        cursor: Option<f64>,
        limit: usize,
    ) -> Result<Vec<(String, f64)>, DynError> {
        let result = Event::try_from_index_sorted_set(
            &["Events"],
            cursor,
            None,
            None,
            Some(limit + 1),
            SortOrder::Descending,
            None,
        )
        .await;

        let result = match result {
            Ok(r) => r,
            Err(error) => {
                error!(
                    "IndexReadFailed: Failed to read from index due to Redis error: {}",
                    error
                );
                return Err(error);
            }
        };

        match result {
            Some(v) => Ok(v),
            None => Ok(Vec::new()),
        }
    }
}
