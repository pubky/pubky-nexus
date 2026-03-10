use async_trait::async_trait;
use chrono::Utc;
use nexus_common::db::kv::RedisResult;
use pubky_app_specs::ParsedUri;
use serde::{Deserialize, Serialize};

use nexus_common::db::RedisOps;

use crate::events::EventProcessorError;

pub const RETRY_MANAGER_PREFIX: &str = "RetryManager";
pub const RETRY_MANAGER_EVENTS_INDEX: [&str; 1] = ["events"];
pub const RETRY_MANAGER_STATE_INDEX: [&str; 1] = ["state"];

/// Represents an event in the retry queue and it is used to manage events that have failed
/// to process and need to be retried
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryEvent {
    /// Retry attempts made for this event
    pub retry_count: u32,
    /// The type of error that caused the event to fail
    /// This determines how the event should be processed during the retry process
    pub error_type: EventProcessorError,
}

#[async_trait]
impl RedisOps for RetryEvent {
    async fn prefix() -> String {
        String::from(RETRY_MANAGER_PREFIX)
    }
}

impl RetryEvent {
    pub fn new(error_type: EventProcessorError) -> Self {
        Self {
            retry_count: 0,
            error_type,
        }
    }

    /// It processes a homeserver URI and extracts specific components to form a index key
    /// in the format `"{pubkyId}:{repository_model}:{event_id}"`
    /// # Parameters
    /// - `event_uri`: A string slice representing the event URI to be processed
    pub fn generate_index_key(event_uri: &str) -> Option<String> {
        let parsed_uri = match ParsedUri::try_from(event_uri) {
            Ok(parsed_uri) => parsed_uri,
            Err(_) => return None,
        };

        let user_id = parsed_uri.user_id;
        let key = match parsed_uri.resource.id() {
            Some(id) => format!("{}:{}:{}", user_id, parsed_uri.resource, id),
            None => format!("{}:{}", user_id, parsed_uri.resource),
        };

        Some(key)
    }

    pub fn generate_index_key_from_uri(event_uri: &ParsedUri) -> String {
        let user_id = &event_uri.user_id;
        let event_resource = &event_uri.resource;

        match event_uri.resource.id() {
            Some(id) => format!("{user_id}:{event_resource}:{id}"),
            None => format!("{user_id}:{event_resource}"),
        }
    }

    /// Stores an event in both a sorted set and a JSON index in Redis.
    /// It adds an event line to a Redis sorted set with a timestamp-based score
    /// and also stores the event details in a separate JSON index for retrieval.
    /// # Arguments
    /// * `event_line` - A `String` representing the event line to be indexed.
    pub async fn put_to_index(&self, event_line: String) -> RedisResult<()> {
        Self::put_index_sorted_set(
            &RETRY_MANAGER_EVENTS_INDEX,
            // NOTE: Don't know if we should use now timestamp or the event timestamp
            &[(Utc::now().timestamp_millis() as f64, &event_line)],
            Some(RETRY_MANAGER_PREFIX),
            None,
        )
        .await?;

        let index = &[RETRY_MANAGER_STATE_INDEX, [&event_line]].concat();
        self.put_index_json(index, None, None).await?;

        Ok(())
    }

    /// Checks if a specific event exists in the Redis sorted set
    /// # Arguments
    /// * `event_index` - A `&str` representing the event index to check
    pub async fn check_uri(event_index: &str) -> Result<Option<isize>, EventProcessorError> {
        Self::check_sorted_set_member(
            Some(RETRY_MANAGER_PREFIX),
            &RETRY_MANAGER_EVENTS_INDEX,
            &[event_index],
        )
        .await
        .map_err(|e| {
            EventProcessorError::InternalError(format!(
                "Could not check uri for event: {event_index}, reason {e}"
            ))
        })
    }

    /// Retrieves an event from the JSON index in Redis based on its index
    /// # Arguments
    /// * `event_index` - A `&str` representing the event index to retrieve
    pub async fn get_from_index(event_index: &str) -> RedisResult<Option<Self>> {
        let index: &Vec<&str> = &[RETRY_MANAGER_STATE_INDEX, [event_index]].concat();
        Self::try_from_index_json(index, None).await
    }
}
