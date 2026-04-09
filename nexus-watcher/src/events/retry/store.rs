use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::debug;

use nexus_common::models::event::EventProcessorError;

use super::RetryEvent;

/// Storage backend for [`RetryEvent`]s.
///
/// Abstracts persistence so the processor can run against Redis in production and
/// against a per-test in-memory store under `cargo test`, keeping parallel tests
/// from stomping on each other's queue state.
#[async_trait]
pub trait RetryStore: Send + Sync {
    /// Insert or replace the event stored under `resource_key`.
    async fn put(&self, resource_key: &str, event: &RetryEvent) -> Result<(), EventProcessorError>;

    /// Retrieve the event for `resource_key`, if any.
    async fn get(&self, resource_key: &str) -> Result<Option<RetryEvent>, EventProcessorError>;

    /// Remove `resource_key` from the store. No-op if absent.
    async fn remove(&self, resource_key: &str) -> Result<(), EventProcessorError>;

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
    ) -> Result<Vec<(String, RetryEvent)>, EventProcessorError>;
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
    async fn put(&self, resource_key: &str, event: &RetryEvent) -> Result<(), EventProcessorError> {
        event.put_to_index(resource_key).await?;
        Ok(())
    }

    async fn get(&self, resource_key: &str) -> Result<Option<RetryEvent>, EventProcessorError> {
        Ok(RetryEvent::get_from_index(resource_key).await?)
    }

    async fn remove(&self, resource_key: &str) -> Result<(), EventProcessorError> {
        RetryEvent::remove_from_index(resource_key).await?;
        Ok(())
    }

    async fn fetch_ready(
        &self,
        now: i64,
        limit: Option<usize>,
    ) -> Result<Vec<(String, RetryEvent)>, EventProcessorError> {
        let key_score_pairs = match RetryEvent::fetch_ready(now, limit).await? {
            Some(pairs) => pairs,
            None => return Ok(Vec::new()),
        };

        let mut events = Vec::with_capacity(key_score_pairs.len());
        for (key, _score) in key_score_pairs {
            match RetryEvent::get_from_index(&key).await? {
                Some(event) => events.push((key, event)),
                None => {
                    // Sorted-set entry with no JSON state — tombstone, clean up and skip.
                    debug!("Stale retry entry detected for key {}, cleaning up", key);
                    RetryEvent::remove_from_index(&key).await?;
                }
            }
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
    inner: Mutex<HashMap<String, RetryEvent>>,
}

impl InMemoryRetryStore {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Convenience constructor returning an `Arc` already erased to `dyn RetryStore`,
    /// the form the [`super::RetryProcessor`] expects.
    pub fn arc() -> Arc<dyn RetryStore> {
        Arc::new(Self::new())
    }
}

impl Default for InMemoryRetryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RetryStore for InMemoryRetryStore {
    async fn put(&self, resource_key: &str, event: &RetryEvent) -> Result<(), EventProcessorError> {
        self.inner
            .lock()
            .await
            .insert(resource_key.to_string(), event.clone());
        Ok(())
    }

    async fn get(&self, resource_key: &str) -> Result<Option<RetryEvent>, EventProcessorError> {
        Ok(self.inner.lock().await.get(resource_key).cloned())
    }

    async fn remove(&self, resource_key: &str) -> Result<(), EventProcessorError> {
        self.inner.lock().await.remove(resource_key);
        Ok(())
    }

    async fn fetch_ready(
        &self,
        now: i64,
        limit: Option<usize>,
    ) -> Result<Vec<(String, RetryEvent)>, EventProcessorError> {
        let guard = self.inner.lock().await;
        let mut ready: Vec<(String, RetryEvent)> = guard
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
