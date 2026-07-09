use crate::events::{Event, EventType};
use async_trait::async_trait;
use chrono::Utc;
use deadpool_redis::redis::Script;
use nexus_common::db::kv::{RedisError, RedisResult, SortOrder};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

use nexus_common::db::{get_redis_conn, RedisOps};

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
    /// Identity of this logical enqueue, used by the compare-and-act store ops.
    ///
    /// Retry entries are keyed by hash(URI) only, so a newer event for the same
    /// URI overwrites the stored entry while the retry processor may still hold
    /// the previous one in memory. Conditional removals and reschedules compare
    /// this nonce so a stale in-flight retry cannot clobber the newer entry.
    ///
    /// A reschedule keeps the nonce (same logical event); only a fresh enqueue
    /// mints a new one. `#[serde(default)]` lets entries stored before this
    /// field existed deserialize as nonce 0, which the conditional ops also
    /// treat as the "field missing in stored JSON" case.
    #[serde(default)]
    pub nonce: i64,
}

#[async_trait]
impl RedisOps for RetryEvent {
    async fn prefix() -> String {
        String::from(RETRY_MANAGER_PREFIX)
    }
}

/// Shared Lua preamble for the compare-and-act scripts
/// ([`RetryEvent::remove_from_index_if_nonce`] and
/// [`RetryEvent::put_to_index_if_nonce`]): loads the stored entry's nonce from
/// the JSON state at `KEYS[1]` and returns 0 (no-op) unless it equals
/// `ARGV[1]`; each script appends its own action suffix.
///
/// Nonces are compared as raw strings: they are i64 nanosecond timestamps that
/// would lose precision as Lua doubles. `JSON.GET key $.nonce` returns a JSON
/// array of matches, e.g. `[123]`; an entry stored before the nonce field
/// existed yields `[]` and is treated as nonce 0, matching `#[serde(default)]`
/// on the Rust side.
const NONCE_GUARD_LUA: &str = r#"
            local stored = redis.call('JSON.GET', KEYS[1], '$.nonce')
            if not stored then
                return 0
            end
            local nonce = string.match(stored, '^%[(%-?%d+)%]$') or '0'
            if nonce ~= ARGV[1] then
                return 0
            end"#;

/// Script for [`RetryEvent::remove_from_index_if_nonce`]. Static so the source
/// string and its SHA1 are computed once, not on every call.
static REMOVE_IF_NONCE_SCRIPT: LazyLock<Script> = LazyLock::new(|| {
    Script::new(&format!(
        r#"{NONCE_GUARD_LUA}
            redis.call('JSON.DEL', KEYS[1])
            redis.call('ZREM', KEYS[2], ARGV[2])
            return 1
        "#
    ))
});

/// Script for [`RetryEvent::put_to_index_if_nonce`]. Static so the source
/// string and its SHA1 are computed once, not on every call.
static PUT_IF_NONCE_SCRIPT: LazyLock<Script> = LazyLock::new(|| {
    Script::new(&format!(
        r#"{NONCE_GUARD_LUA}
            redis.call('JSON.SET', KEYS[1], '$', ARGV[2])
            redis.call('ZADD', KEYS[2], ARGV[3], ARGV[4])
            return 1
        "#
    ))
});

impl RetryEvent {
    /// Creates a new RetryEvent from the source event
    pub fn new(event: &Event, next_retry_at: i64, origin_homeserver_id: impl Into<String>) -> Self {
        Self {
            retry_count: 0,
            event_type: event.event_type.clone(),
            event_uri: event.uri.clone(),
            next_retry_at,
            origin_homeserver_id: origin_homeserver_id.into(),
            nonce: Self::fresh_nonce(),
        }
    }

    /// Nonce for a freshly enqueued event: the current wall-clock time in
    /// nanoseconds, falling back to milliseconds if nanoseconds do not fit in
    /// an i64 (dates past the year 2262).
    fn fresh_nonce() -> i64 {
        let now = Utc::now();
        now.timestamp_nanos_opt()
            .unwrap_or_else(|| now.timestamp_millis())
    }

    /// Full Redis key of the JSON state entry for `index_key`, matching the
    /// layout used by the RedisOps helpers (prefix + key parts joined with
    /// ':').
    fn state_json_key(index_key: &IndexKey) -> String {
        format!(
            "{RETRY_MANAGER_PREFIX}:{}:{}",
            RETRY_MANAGER_STATE_INDEX[0],
            index_key.as_str()
        )
    }

    /// Full Redis key of the events sorted set, matching the layout used by
    /// the RedisOps helpers.
    fn events_sorted_set_key() -> String {
        format!("{RETRY_MANAGER_PREFIX}:{}", RETRY_MANAGER_EVENTS_INDEX[0])
    }

    /// Stores an event in both a sorted set and a JSON index in Redis.
    /// The sorted set uses next_retry_at as the score for efficient retrieval of ready events.
    ///
    /// Both keys are written in a single MULTI/EXEC transaction. Writing them
    /// with two separate commands would open a race with the conditional ops:
    /// a stale [`Self::remove_from_index_if_nonce`] could interleave between
    /// them, match the old JSON nonce, and delete the sorted-set member this
    /// enqueue just wrote, leaving JSON state that [`Self::fetch_ready`] never
    /// returns.
    #[tracing::instrument(name = "retry.index.write", skip_all)]
    pub async fn put_to_index(&self, index_key: &IndexKey) -> RedisResult<()> {
        let payload = serde_json::to_string(self)
            .map_err(|e| RedisError::SerializationFailed(Box::new(e)))?;

        let mut pipe = deadpool_redis::redis::pipe();
        pipe.atomic()
            // Store full RetryEvent struct in JSON
            .cmd("JSON.SET")
            .arg(Self::state_json_key(index_key))
            .arg("$")
            .arg(payload)
            .ignore()
            // Add to sorted set with next_retry_at as score
            .cmd("ZADD")
            .arg(Self::events_sorted_set_key())
            .arg(self.next_retry_at)
            .arg(index_key.as_str())
            .ignore();

        let mut redis_conn = get_redis_conn().await?;
        let _: () = pipe
            .query_async(&mut redis_conn)
            .await
            .map_err(RedisError::from)?;
        Ok(())
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
    ///
    /// NOTE: the two removals are not atomic, so a concurrent enqueue for the
    /// same URI can interleave between them and lose its entry. This is the
    /// last remaining non-atomic writer of the key pair; the processor only
    /// removes fetched events via [`Self::remove_from_index_if_nonce`], and
    /// this baseline primitive currently has no production callers.
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

    /// Atomically removes the event for `index_key` only if the stored entry's
    /// nonce equals `expected_nonce`. Returns whether the removal happened.
    ///
    /// The compare and the mutation run in a single Lua script so a concurrent
    /// enqueue for the same URI (which overwrites the entry with a fresh nonce)
    /// cannot be deleted by a stale in-flight retry. Nonce-comparison rules
    /// live in [`NONCE_GUARD_LUA`].
    #[tracing::instrument(name = "retry.index.remove_if", skip_all)]
    pub async fn remove_from_index_if_nonce(
        index_key: &IndexKey,
        expected_nonce: i64,
    ) -> RedisResult<bool> {
        let mut redis_conn = get_redis_conn().await?;
        let removed: i64 = REMOVE_IF_NONCE_SCRIPT
            .key(Self::state_json_key(index_key))
            .key(Self::events_sorted_set_key())
            .arg(expected_nonce.to_string())
            .arg(index_key.as_str())
            .invoke_async(&mut redis_conn)
            .await
            .map_err(RedisError::from)?;
        Ok(removed == 1)
    }

    /// Atomically replaces the event for `index_key` only if the stored entry's
    /// nonce equals `expected_nonce`. Returns whether the write happened.
    ///
    /// Same key layout and nonce-comparison rules ([`NONCE_GUARD_LUA`]) as
    /// [`Self::remove_from_index_if_nonce`]; on a match it performs the same
    /// JSON.SET + ZADD as [`Self::put_to_index`].
    #[tracing::instrument(name = "retry.index.put_if", skip_all)]
    pub async fn put_to_index_if_nonce(
        &self,
        index_key: &IndexKey,
        expected_nonce: i64,
    ) -> RedisResult<bool> {
        let payload = serde_json::to_string(self)
            .map_err(|e| RedisError::SerializationFailed(Box::new(e)))?;

        let mut redis_conn = get_redis_conn().await?;
        let updated: i64 = PUT_IF_NONCE_SCRIPT
            .key(Self::state_json_key(index_key))
            .key(Self::events_sorted_set_key())
            .arg(expected_nonce.to_string())
            .arg(payload)
            .arg(self.next_retry_at)
            .arg(index_key.as_str())
            .invoke_async(&mut redis_conn)
            .await
            .map_err(RedisError::from)?;
        Ok(updated == 1)
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

    #[test]
    fn nonce_defaults_to_zero_for_entries_stored_before_the_field_existed() {
        // JSON shape of a RetryEvent enqueued before the nonce field was added.
        let json = r#"{
            "retry_count": 3,
            "event_type": "Put",
            "event_uri": "pubky://abc123/pub/pubky.app/posts/xyz789",
            "next_retry_at": 1234567890,
            "origin_homeserver_id": "hs_id"
        }"#;
        let event: RetryEvent = serde_json::from_str(json).expect("pre-nonce JSON must parse");
        assert_eq!(event.nonce, 0);
    }
}
