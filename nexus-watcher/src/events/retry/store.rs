use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::debug;

use crate::errors::EventProcessorError;

use super::{IndexKey, RetryEvent};

/// Storage backend for [`RetryEvent`]s.
///
/// Abstracts persistence so the processor can run against Redis in production and
/// against a per-test in-memory store under `cargo test`, keeping parallel tests
/// from stomping on each other's queue state.
///
/// Multi-replica ABA bound: reschedules keep the entry's nonce (they update the
/// same logical event), so with multiple concurrent `RetryProcessor` replicas a
/// stale reschedule can regress `retry_count` of the same logical event, but it
/// can never resurrect a removed entry nor clobber a different event, because
/// fresh enqueues always mint new nonces.
#[async_trait]
pub trait RetryStore: Send + Sync {
    /// Insert or replace `event`, keyed by `IndexKey::for_uri(&event.event_uri)`.
    async fn put(&self, event: &RetryEvent) -> Result<(), EventProcessorError>;

    /// Retrieve the event for `index_key`, if any.
    async fn get(&self, index_key: &IndexKey) -> Result<Option<RetryEvent>, EventProcessorError>;

    /// Remove `index_key` from the store. No-op if absent.
    ///
    /// Unconditional baseline primitive. The processor currently drives every
    /// removal of a fetched event through [`Self::remove_if`], including the
    /// corrupted-entry cleanup: the fetched entry deserialized fine, so its
    /// nonce is trustworthy even when the reconstructed event line fails to
    /// parse. Reach for this only when there is no fetched entry to compare
    /// against.
    async fn remove(&self, index_key: &IndexKey) -> Result<(), EventProcessorError>;

    /// Remove `index_key` only if the stored entry's nonce equals
    /// `expected_nonce`. The compare and the removal are atomic.
    ///
    /// Returns `true` when the entry was removed, `false` when nothing was done
    /// because the entry is absent or carries a different nonce, i.e. it was
    /// superseded by a newer event enqueued for the same URI.
    async fn remove_if(
        &self,
        index_key: &IndexKey,
        expected_nonce: i64,
    ) -> Result<bool, EventProcessorError>;

    /// Insert or replace the entry keyed by `IndexKey::for_uri(&event.event_uri)`
    /// only if the currently stored entry's nonce equals `expected_nonce`. The
    /// compare and the write are atomic.
    ///
    /// Returns `true` when the write happened, `false` when nothing was done
    /// because the entry is absent or carries a different nonce.
    async fn put_if(
        &self,
        event: &RetryEvent,
        expected_nonce: i64,
    ) -> Result<bool, EventProcessorError>;

    /// Return all events with `next_retry_at <= now`, ordered ascending by
    /// `next_retry_at`, capped at `limit` if provided.
    ///
    /// Implementations are responsible for cleaning up any internal inconsistencies
    /// (e.g. Redis sorted set entries that point at missing JSON state), so the
    /// caller always receives fully-resolved `(key, event)` pairs.
    async fn fetch_ready(
        &self,
        now: i64,
        limit: Option<usize>,
    ) -> Result<Vec<(IndexKey, RetryEvent)>, EventProcessorError>;
}

/// Redis-backed [`RetryStore`], delegating to the `RetryEvent::*` helpers that
/// wrap the Redis sorted-set + JSON-state layout.
pub struct RedisRetryStore;

impl RedisRetryStore {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RedisRetryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RetryStore for RedisRetryStore {
    async fn put(&self, event: &RetryEvent) -> Result<(), EventProcessorError> {
        let index_key = IndexKey::for_uri(&event.event_uri);
        event.put_to_index(&index_key).await?;
        Ok(())
    }

    async fn get(&self, index_key: &IndexKey) -> Result<Option<RetryEvent>, EventProcessorError> {
        Ok(RetryEvent::get_from_index(index_key).await?)
    }

    async fn remove(&self, index_key: &IndexKey) -> Result<(), EventProcessorError> {
        RetryEvent::remove_from_index(index_key).await?;
        Ok(())
    }

    async fn remove_if(
        &self,
        index_key: &IndexKey,
        expected_nonce: i64,
    ) -> Result<bool, EventProcessorError> {
        Ok(RetryEvent::remove_from_index_if_nonce(index_key, expected_nonce).await?)
    }

    async fn put_if(
        &self,
        event: &RetryEvent,
        expected_nonce: i64,
    ) -> Result<bool, EventProcessorError> {
        let index_key = IndexKey::for_uri(&event.event_uri);
        Ok(event
            .put_to_index_if_nonce(&index_key, expected_nonce)
            .await?)
    }

    async fn fetch_ready(
        &self,
        now: i64,
        limit: Option<usize>,
    ) -> Result<Vec<(IndexKey, RetryEvent)>, EventProcessorError> {
        let key_score_pairs = RetryEvent::fetch_ready(now, limit).await?;

        // Batch-fetch JSON state for every candidate in a single JSON.MGET.
        let keys: Vec<IndexKey> = key_score_pairs.iter().map(|(k, _)| k.clone()).collect();
        let maybe_events = RetryEvent::get_multiple_from_index(&keys).await?;

        let mut events = Vec::with_capacity(key_score_pairs.len());
        let mut stale: Vec<IndexKey> = Vec::new();
        for ((index_key, _score), maybe_event) in key_score_pairs.into_iter().zip(maybe_events) {
            match maybe_event {
                Some(event) => events.push((index_key, event)),
                None => {
                    // Sorted-set entry with no JSON state — tombstone, clean up and skip.
                    debug!("Stale retry entry detected for key {index_key}, cleaning up");
                    stale.push(index_key);
                }
            }
        }

        if !stale.is_empty() {
            RetryEvent::remove_stale_index_entries(&stale).await?;
        }

        Ok(events)
    }
}

/// In-memory [`RetryStore`] intended for unit/integration tests that want
/// per-test isolation without spinning up Redis state.
///
/// Each instance is independent, so parallel tests that each own their own
/// `InMemoryRetryStore` cannot observe or mutate each other's events.
pub struct InMemoryRetryStore {
    inner: Mutex<HashMap<IndexKey, RetryEvent>>,
}

impl InMemoryRetryStore {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryRetryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RetryStore for InMemoryRetryStore {
    async fn put(&self, event: &RetryEvent) -> Result<(), EventProcessorError> {
        let index_key = IndexKey::for_uri(&event.event_uri);
        self.inner.lock().await.insert(index_key, event.clone());
        Ok(())
    }

    async fn get(&self, index_key: &IndexKey) -> Result<Option<RetryEvent>, EventProcessorError> {
        Ok(self.inner.lock().await.get(index_key).cloned())
    }

    async fn remove(&self, index_key: &IndexKey) -> Result<(), EventProcessorError> {
        self.inner.lock().await.remove(index_key);
        Ok(())
    }

    async fn remove_if(
        &self,
        index_key: &IndexKey,
        expected_nonce: i64,
    ) -> Result<bool, EventProcessorError> {
        let mut guard = self.inner.lock().await;
        match guard.get(index_key) {
            Some(stored) if stored.nonce == expected_nonce => {
                guard.remove(index_key);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    async fn put_if(
        &self,
        event: &RetryEvent,
        expected_nonce: i64,
    ) -> Result<bool, EventProcessorError> {
        let index_key = IndexKey::for_uri(&event.event_uri);
        let mut guard = self.inner.lock().await;
        match guard.get(&index_key) {
            Some(stored) if stored.nonce == expected_nonce => {
                guard.insert(index_key, event.clone());
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    async fn fetch_ready(
        &self,
        now: i64,
        limit: Option<usize>,
    ) -> Result<Vec<(IndexKey, RetryEvent)>, EventProcessorError> {
        let guard = self.inner.lock().await;
        let mut ready: Vec<(IndexKey, RetryEvent)> = guard
            .iter()
            .filter(|(_, event)| event.next_retry_at <= now)
            .map(|(key, event)| (key.clone(), event.clone()))
            .collect();
        // Ascending by (score, key) to match Redis sorted-set semantics
        // (same-score members are ordered lexicographically).
        ready.sort_by(|(key_a, event_a), (key_b, event_b)| {
            event_a
                .next_retry_at
                .cmp(&event_b.next_retry_at)
                .then_with(|| key_a.cmp(key_b))
        });
        if let Some(limit) = limit {
            ready.truncate(limit);
        }
        Ok(ready)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventType;

    const URI: &str = "pubky://test_user/pub/pubky.app/posts/0000000001";

    fn retry_event(event_type: EventType, nonce: i64) -> RetryEvent {
        RetryEvent {
            retry_count: 0,
            event_type,
            event_uri: URI.to_string(),
            next_retry_at: 0,
            origin_homeserver_id: "test_hs".to_string(),
            nonce,
        }
    }

    #[tokio::test]
    async fn remove_if_with_stale_nonce_leaves_newer_entry() {
        let store = InMemoryRetryStore::new();
        let key = IndexKey::for_uri(URI);

        store.put(&retry_event(EventType::Put, 1)).await.unwrap();
        // A newer event for the same URI overwrites the entry.
        store.put(&retry_event(EventType::Del, 2)).await.unwrap();

        // Conditional removal keyed on the old nonce must be a no-op.
        assert!(!store.remove_if(&key, 1).await.unwrap());
        let stored = store.get(&key).await.unwrap().expect("entry must survive");
        assert_eq!(stored.nonce, 2);
    }

    #[tokio::test]
    async fn put_if_with_stale_nonce_does_not_clobber() {
        let store = InMemoryRetryStore::new();
        let key = IndexKey::for_uri(URI);

        store.put(&retry_event(EventType::Del, 2)).await.unwrap();

        // A reschedule of the older (nonce 1) event must not overwrite.
        let mut rescheduled = retry_event(EventType::Put, 1);
        rescheduled.retry_count = 5;
        rescheduled.next_retry_at = 999;
        assert!(!store.put_if(&rescheduled, 1).await.unwrap());

        let stored = store.get(&key).await.unwrap().expect("entry must survive");
        assert_eq!(stored.nonce, 2);
        assert_eq!(stored.retry_count, 0);
        assert_eq!(stored.event_type, EventType::Del);
    }

    #[tokio::test]
    async fn remove_if_with_matching_nonce_removes_and_returns_true() {
        let store = InMemoryRetryStore::new();
        let key = IndexKey::for_uri(URI);

        store.put(&retry_event(EventType::Put, 7)).await.unwrap();
        assert!(store.remove_if(&key, 7).await.unwrap());
        assert!(store.get(&key).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn put_if_with_matching_nonce_writes_and_returns_true() {
        let store = InMemoryRetryStore::new();
        let key = IndexKey::for_uri(URI);

        store.put(&retry_event(EventType::Put, 7)).await.unwrap();

        let mut rescheduled = retry_event(EventType::Put, 7);
        rescheduled.retry_count = 1;
        rescheduled.next_retry_at = 42;
        assert!(store.put_if(&rescheduled, 7).await.unwrap());

        let stored = store.get(&key).await.unwrap().expect("entry must exist");
        assert_eq!(stored.retry_count, 1);
        assert_eq!(stored.next_retry_at, 42);
        assert_eq!(stored.nonce, 7);
    }

    #[tokio::test]
    async fn conditional_ops_on_absent_entry_are_noops() {
        let store = InMemoryRetryStore::new();
        let key = IndexKey::for_uri(URI);

        assert!(!store.remove_if(&key, 0).await.unwrap());
        assert!(!store
            .put_if(&retry_event(EventType::Put, 1), 1)
            .await
            .unwrap());
        assert!(store.get(&key).await.unwrap().is_none());
    }

    /// The #963 clobber scenario at store level: the retry processor holds a
    /// fetched PUT (E1) while a live processor enqueues a DEL (E2) for the same
    /// URI, overwriting the hash(URI)-keyed entry. E1's success-path removal
    /// must not delete E2, which must remain fetchable.
    #[tokio::test]
    async fn stale_removal_does_not_lose_newer_event_end_to_end() {
        let store = InMemoryRetryStore::new();
        let key = IndexKey::for_uri(URI);

        // E1 = PUT enqueued, then snapshotted by the retry processor.
        let e1 = retry_event(EventType::Put, 100);
        store.put(&e1).await.unwrap();

        // E2 = DEL for the same URI fails transiently on a live processor and
        // is enqueued with a fresh nonce, overwriting E1's slot.
        let e2 = retry_event(EventType::Del, 200);
        store.put(&e2).await.unwrap();

        // E1's retry succeeds; its conditional removal must be a no-op.
        assert!(!store.remove_if(&key, e1.nonce).await.unwrap());

        // E2 is still there and still returned by fetch_ready.
        let ready = store.fetch_ready(i64::MAX, None).await.unwrap();
        let (_, fetched) = ready
            .iter()
            .find(|(k, _)| k == &key)
            .expect("E2 must still be fetchable");
        assert_eq!(fetched.nonce, 200);
        assert_eq!(fetched.event_type, EventType::Del);
    }
}
