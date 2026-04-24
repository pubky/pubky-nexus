use async_trait::async_trait;
use nexus_common::db::kv::{RedisResult, SortOrder};
use nexus_common::models::event::EventType;
use serde::{Deserialize, Serialize};

/// The URI of the event's resource, used as the unique key in the retry index.
pub type RetryEventIndexKey = String;

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

    /// Stores an event in both a sorted set and a JSON index in Redis.
    /// The sorted set uses next_retry_at as the score for efficient retrieval of ready events.
    /// # Arguments
    /// * `index_key` - A `&RetryEventIndexKey` representing the index key (used as member in sorted set and JSON key)
    #[tracing::instrument(name = "retry.index.write", skip_all)]
    pub async fn put_to_index(&self, index_key: &RetryEventIndexKey) -> RedisResult<()> {
        // Add to sorted set with next_retry_at as score
        Self::put_index_sorted_set(
            &RETRY_MANAGER_EVENTS_INDEX,
            &[(self.next_retry_at as f64, index_key.as_str())],
            Some(RETRY_MANAGER_PREFIX),
            None,
        )
        .await?;

        // Store full RetryEvent struct in JSON
        let index = &[RETRY_MANAGER_STATE_INDEX[0], index_key.as_str()];
        self.put_index_json(index, None, None).await?;

        Ok(())
    }

    /// Checks if a specific event exists in the Redis sorted set.
    ///
    /// Only used by integration tests (`nexus-watcher/tests/`); kept `pub` because
    /// those tests compile against this crate as an external consumer.
    /// # Arguments
    /// * `index_key` - A `&str` representing the index key to check
    pub async fn check_uri(index_key: &str) -> RedisResult<bool> {
        Self::check_sorted_set_member(
            Some(RETRY_MANAGER_PREFIX),
            &RETRY_MANAGER_EVENTS_INDEX,
            &[index_key],
        )
        .await
        .map(|rank| rank.is_some())
    }

    /// Retrieves an event from the JSON index in Redis based on its index
    /// # Arguments
    /// * `index_key` - A `&RetryEventIndexKey` representing the index key to retrieve
    pub async fn get_from_index(index_key: &RetryEventIndexKey) -> RedisResult<Option<Self>> {
        let index = &[RETRY_MANAGER_STATE_INDEX[0], index_key.as_str()];
        Self::try_from_index_json(index, None).await
    }

    /// Batched variant of [`Self::get_from_index`] backed by a single `JSON.MGET`.
    ///
    /// Results are returned positionally: element `i` corresponds to `index_keys[i]`,
    /// with `None` for keys whose JSON state is missing (tombstones).
    pub async fn get_multiple_from_index(index_keys: &[&str]) -> RedisResult<Vec<Option<Self>>> {
        let key_parts: Vec<[&str; 2]> = index_keys
            .iter()
            .map(|k| [RETRY_MANAGER_STATE_INDEX[0], *k])
            .collect();
        let key_parts_refs: Vec<&[&str]> = key_parts.iter().map(|p| p.as_slice()).collect();
        Self::try_from_index_multiple_json(&key_parts_refs).await
    }

    /// Removes an event from the retry queue (both sorted set and JSON state)
    /// # Arguments
    /// * `index_key` - A `&RetryEventIndexKey` representing the index key to remove
    #[tracing::instrument(name = "retry.index.remove", skip_all)]
    pub async fn remove_from_index(index_key: &RetryEventIndexKey) -> RedisResult<()> {
        // Remove from sorted set
        Self::remove_from_index_sorted_set(
            Some(RETRY_MANAGER_PREFIX),
            &RETRY_MANAGER_EVENTS_INDEX,
            &[index_key.as_str()],
        )
        .await?;

        // Remove JSON state
        let index = &[RETRY_MANAGER_STATE_INDEX[0], index_key.as_str()];
        Self::remove_from_index_multiple_json(&[index.as_slice()]).await?;

        Ok(())
    }

    /// Removes multiple sorted-set index entries without touching JSON state.
    ///
    /// Used for tombstone cleanup in the retry store: the JSON state is already
    /// missing, so a single batched ZREM reconciles the index.
    #[tracing::instrument(name = "retry.index.remove_stale", skip_all)]
    pub async fn remove_stale_index_entries(index_keys: &[&str]) -> RedisResult<()> {
        Self::remove_from_index_sorted_set(
            Some(RETRY_MANAGER_PREFIX),
            &RETRY_MANAGER_EVENTS_INDEX,
            index_keys,
        )
        .await
    }

    /// Fetches events from the retry queue that are ready to be retried (next_retry_at <= now)
    /// # Arguments
    /// * `now` - Current time in milliseconds since epoch
    /// * `limit` - Maximum number of events to fetch per batch
    /// # Returns
    /// A vector of (index_key, score) pairs; empty when no events are ready.
    #[tracing::instrument(name = "retry.index.fetch_ready", skip_all)]
    pub async fn fetch_ready(
        now: i64,
        limit: Option<usize>,
    ) -> Result<Vec<(RetryEventIndexKey, f64)>, EventProcessorError> {
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
        .map(Option::unwrap_or_default)
        .map_err(|e| EventProcessorError::generic(format!("Failed to fetch retry events: {}", e)))
    }
}
