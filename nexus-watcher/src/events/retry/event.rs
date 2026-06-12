use async_trait::async_trait;
use nexus_common::db::kv::{RedisResult, SortOrder};
use nexus_common::models::event::{Event, EventType};
use serde::{Deserialize, Serialize};

use nexus_common::db::RedisOps;

use crate::events::EventProcessorError;

// v2: RetryEvent schema changed incompatibly; bumped prefix so old keys under
// "RetryManager:*" stay orphaned rather than failing to deserialize.
pub const RETRY_MANAGER_PREFIX: &str = "RetryManagerV2";
pub const RETRY_MANAGER_EVENTS_INDEX: [&str; 1] = ["events"];
pub const RETRY_MANAGER_STATE_INDEX: [&str; 1] = ["state"];

// Constructed via for_uri (hash a URI) or from_stored (trust a value already in the
// Redis sorted set), so a raw unhashed URI can't be used as a key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IndexKey(String);

impl IndexKey {
    pub fn for_uri(uri: &str) -> Self {
        Self(nexus_common::utils::hash_str_hex(uri))
    }

    /// Wraps a value already stored as a member of the events sorted set.
    fn from_stored(key: String) -> Self {
        Self(key)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for IndexKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

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
    /// Homeserver that served the event
    pub origin_homeserver_id: String,
}

#[async_trait]
impl RedisOps for RetryEvent {
    async fn prefix() -> String {
        String::from(RETRY_MANAGER_PREFIX)
    }
}

impl RetryEvent {
    /// Creates a new RetryEvent from the source event
    pub fn new(event: &Event, next_retry_at: i64, origin_homeserver_id: impl Into<String>) -> Self {
        Self {
            retry_count: 0,
            event_type: event.event_type.clone(),
            event_uri: event.uri.clone(),
            next_retry_at,
            origin_homeserver_id: origin_homeserver_id.into(),
        }
    }

    /// Stores an event in both a sorted set and a JSON index in Redis.
    /// The sorted set uses next_retry_at as the score for efficient retrieval of ready events.
    #[tracing::instrument(name = "retry.index.write", skip_all)]
    pub async fn put_to_index(&self, index_key: &IndexKey) -> RedisResult<()> {
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
        self.put_index_json(index, None, None).await
    }

    /// Checks if a specific event exists in the Redis sorted set.
    ///
    /// Only used by integration tests (`nexus-watcher/tests/`); kept `pub` because
    /// those tests compile against this crate as an external consumer.
    pub async fn check_index_key(index_key: &IndexKey) -> RedisResult<bool> {
        Self::check_sorted_set_member(
            Some(RETRY_MANAGER_PREFIX),
            &RETRY_MANAGER_EVENTS_INDEX,
            &[index_key.as_str()],
        )
        .await
        .map(|rank| rank.is_some())
    }

    /// Retrieves an event from the JSON index in Redis based on its index
    #[tracing::instrument(name = "retry.index.get", skip_all)]
    pub async fn get_from_index(index_key: &IndexKey) -> RedisResult<Option<Self>> {
        let index = &[RETRY_MANAGER_STATE_INDEX[0], index_key.as_str()];
        Self::try_from_index_json(index, None).await
    }

    /// Batched variant of [`Self::get_from_index`] backed by a single `JSON.MGET`.
    ///
    /// Results are returned positionally: element `i` corresponds to `index_keys[i]`,
    /// with `None` for keys whose JSON state is missing (tombstones).
    #[tracing::instrument(name = "retry.index.get_multiple", skip_all)]
    pub async fn get_multiple_from_index(
        index_keys: &[IndexKey],
    ) -> RedisResult<Vec<Option<Self>>> {
        let key_parts: Vec<[&str; 2]> = index_keys
            .iter()
            .map(|k| [RETRY_MANAGER_STATE_INDEX[0], k.as_str()])
            .collect();
        let key_parts_refs: Vec<&[&str]> = key_parts.iter().map(|p| p.as_slice()).collect();
        Self::try_from_index_multiple_json(&key_parts_refs).await
    }

    /// Removes an event from the retry queue (both sorted set and JSON state)
    #[tracing::instrument(name = "retry.index.remove", skip_all)]
    pub async fn remove_from_index(index_key: &IndexKey) -> RedisResult<()> {
        // Remove from sorted set
        Self::remove_from_index_sorted_set(
            Some(RETRY_MANAGER_PREFIX),
            &RETRY_MANAGER_EVENTS_INDEX,
            &[index_key.as_str()],
        )
        .await?;

        // Remove JSON state
        let index = &[RETRY_MANAGER_STATE_INDEX[0], index_key.as_str()];
        Self::remove_from_index_multiple_json(&[index.as_slice()]).await
    }

    /// Removes multiple sorted-set index entries without touching JSON state.
    ///
    /// Used for tombstone cleanup in the retry store: the JSON state is already
    /// missing, so a single batched ZREM reconciles the index.
    #[tracing::instrument(name = "retry.index.remove_stale", skip_all)]
    pub async fn remove_stale_index_entries(index_keys: &[IndexKey]) -> RedisResult<()> {
        let keys: Vec<&str> = index_keys.iter().map(IndexKey::as_str).collect();
        Self::remove_from_index_sorted_set(
            Some(RETRY_MANAGER_PREFIX),
            &RETRY_MANAGER_EVENTS_INDEX,
            &keys,
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
    ) -> Result<Vec<(IndexKey, f64)>, EventProcessorError> {
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
        .map(|pairs| {
            pairs
                .into_iter()
                .map(|(k, s)| (IndexKey::from_stored(k), s))
                .collect()
        })
        .map_err(|e| EventProcessorError::generic(format!("Failed to fetch retry events: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_key_is_32_hex_chars_and_deterministic() {
        let uri = "pubky://abc123/pub/pubky.app/posts/xyz789";
        let key = IndexKey::for_uri(uri);
        assert_eq!(key.as_str().len(), 32);
        assert_eq!(key, IndexKey::for_uri(uri));
    }
}
