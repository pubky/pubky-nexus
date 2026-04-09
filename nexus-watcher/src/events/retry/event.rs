use async_trait::async_trait;
use nexus_common::db::kv::{RedisResult, SortOrder};
use nexus_common::models::event::EventType;
use pubky_app_specs::ParsedUri;
use serde::{Deserialize, Serialize};

use nexus_common::db::RedisOps;

use crate::events::EventProcessorError;

const RETRY_MANAGER_PREFIX: &str = "RetryManager";
const RETRY_MANAGER_EVENTS_INDEX: [&str; 1] = ["events"];
const RETRY_MANAGER_STATE_INDEX: [&str; 1] = ["state"];

/// Represents an event in the retry queue and it is used to manage events that have failed
/// to process and need to be retried
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryEvent {
    /// Retry attempts made for this event
    pub retry_count: u32,
    /// The type of event - needed to reconstruct the event on retry
    pub event_type: EventType,
    /// Original URI - blob is re-fetched on retry
    pub event_uri: String,
    /// Unix ms - when to next attempt (exponential backoff)
    pub next_retry_at: i64,
}

#[async_trait]
impl RedisOps for RetryEvent {
    async fn prefix() -> String {
        String::from(RETRY_MANAGER_PREFIX)
    }
}

impl RetryEvent {
    /// Creates a new RetryEvent
    pub fn new(event_type: EventType, event_uri: String, next_retry_at: i64) -> Self {
        Self {
            retry_count: 0,
            event_type,
            event_uri,
            next_retry_at,
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
    /// The sorted set uses next_retry_at as the score for efficient retrieval of ready events.
    /// # Arguments
    /// * `resource_key` - A `&str` representing the resource key (used as member in sorted set and JSON key)
    #[tracing::instrument(name = "retry.index.write", skip_all)]
    pub async fn put_to_index(&self, resource_key: &str) -> RedisResult<()> {
        // Add to sorted set with next_retry_at as score
        Self::put_index_sorted_set(
            &RETRY_MANAGER_EVENTS_INDEX,
            &[(self.next_retry_at as f64, resource_key)],
            Some(RETRY_MANAGER_PREFIX),
            None,
        )
        .await?;

        // Store full RetryEvent struct in JSON
        let index: &Vec<&str> = &[RETRY_MANAGER_STATE_INDEX, [resource_key]].concat();
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

    /// Removes an event from the retry queue (both sorted set and JSON state)
    /// # Arguments
    /// * `resource_key` - A `&str` representing the resource key to remove
    #[tracing::instrument(name = "retry.index.remove", skip_all)]
    pub async fn remove_from_index(resource_key: &str) -> RedisResult<()> {
        // Remove from sorted set
        Self::remove_from_index_sorted_set(
            Some(RETRY_MANAGER_PREFIX),
            &RETRY_MANAGER_EVENTS_INDEX,
            &[resource_key],
        )
        .await?;

        // Remove JSON state
        let index: &Vec<&str> = &[RETRY_MANAGER_STATE_INDEX, [resource_key]].concat();
        Self::remove_from_index_multiple_json(&[index.as_slice()]).await?;

        Ok(())
    }

    /// Fetches events from the retry queue that are ready to be retried (next_retry_at <= now)
    /// Returns Vec<(resource_key, score)> pairs for events ready for retry
    /// # Arguments
    /// * `now` - Current time in milliseconds since epoch
    /// * `limit` - Maximum number of events to fetch per batch
    /// # Returns
    /// A vector of (resource_key, score) pairs, or None if no events found
    #[tracing::instrument(name = "retry.index.fetch_ready", skip_all)]
    pub async fn fetch_ready(
        now: i64,
        limit: Option<usize>,
    ) -> Result<Option<Vec<(String, f64)>>, EventProcessorError> {
        Self::try_from_index_sorted_set(
            &RETRY_MANAGER_EVENTS_INDEX,
            Some(now as f64), // max_score (start → max in get_range)
            None,             // min_score (end → min in get_range)
            Some(0),          // skip
            limit,
            SortOrder::Ascending,
            Some(RETRY_MANAGER_PREFIX),
        )
        .await
        .map_err(|e| EventProcessorError::generic(format!("Failed to fetch retry events: {}", e)))
    }
}
