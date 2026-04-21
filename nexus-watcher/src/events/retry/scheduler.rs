use std::sync::Arc;

use chrono::Utc;
use tracing::warn;

use nexus_common::models::event::{Event, EventProcessorError};
use nexus_common::WatcherConfig;

use super::{RedisRetryStore, RetryEvent, RetryEventIndexKey, RetryStore};

/// Initial backoff durations applied when an event first lands on the retry queue.
/// Subsequent reschedules use exponential backoff inside [`super::RetryProcessor`].
#[derive(Debug, Clone, Copy)]
pub struct InitialBackoff {
    pub missing_dep_ms: i64,
    pub transient_ms: i64,
}

impl InitialBackoff {
    pub fn from_config(config: &WatcherConfig) -> Self {
        Self {
            missing_dep_ms: config.retry.initial_missing_dep_backoff_secs as i64 * 1000,
            transient_ms: config.retry.initial_backoff_secs as i64 * 1000,
        }
    }
}

/// Enqueues failed events onto the retry queue. Created once per watcher and
/// shared (`Arc`) with every event processor so that processors don't need to
/// carry backoff state themselves.
pub struct RetryScheduler {
    store: Arc<dyn RetryStore>,
    initial: InitialBackoff,
}

impl RetryScheduler {
    pub fn new(store: Arc<dyn RetryStore>, initial: InitialBackoff) -> Self {
        Self { store, initial }
    }

    pub fn from_config(config: &WatcherConfig) -> Self {
        Self::new(
            Arc::new(RedisRetryStore::new()),
            InitialBackoff::from_config(config),
        )
    }

    pub async fn queue_missing_dep(&self, event: &Event) -> Result<(), EventProcessorError> {
        self.enqueue(event, self.initial.missing_dep_ms, "missing dependency")
            .await
    }

    pub async fn queue_transient(&self, event: &Event) -> Result<(), EventProcessorError> {
        self.enqueue(event, self.initial.transient_ms, "client error")
            .await
    }

    /// Returns the transient error initial backoff in milliseconds.
    pub fn transient_backoff_ms(&self) -> i64 {
        self.initial.transient_ms
    }

    /// Returns the missing dependency initial backoff in milliseconds.
    pub fn missing_dep_backoff_ms(&self) -> i64 {
        self.initial.missing_dep_ms
    }

    /// Queues a retry event for an arbitrary URI with a custom backoff.
    /// This is a generic method that does not require a full Event object.
    pub async fn enqueue_raw(
        &self,
        event_type: &nexus_common::models::event::EventType,
        uri: &str,
        backoff_ms: i64,
    ) -> Result<(), EventProcessorError> {
        let key: RetryEventIndexKey = uri.to_owned();

        let next_retry_at = Utc::now().timestamp_millis() + backoff_ms;
        let retry_event = RetryEvent::new(event_type.clone(), uri.to_string(), next_retry_at);

        self.store.put(&key, &retry_event).await?;
        warn!("Queued raw event for retry: {uri}");
        Ok(())
    }

    async fn enqueue(
        &self,
        event: &Event,
        initial_backoff_ms: i64,
        reason: &str,
    ) -> Result<(), EventProcessorError> {
        let key: RetryEventIndexKey = event.uri.clone();

        let next_retry_at = Utc::now().timestamp_millis() + initial_backoff_ms;
        let retry_event =
            RetryEvent::new(event.event_type.clone(), event.uri.clone(), next_retry_at);

        self.store.put(&key, &retry_event).await?;
        warn!("Queued event for retry ({}): {}", reason, event.uri);
        Ok(())
    }
}
