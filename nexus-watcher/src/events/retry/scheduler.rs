use std::sync::Arc;

use chrono::Utc;
use tracing::warn;

use nexus_common::db::PubkyClientError;
use nexus_common::models::event::{Event, EventProcessorError};
use nexus_common::WatcherConfig;

use super::{RedisRetryStore, RetryEvent, RetryStore};

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

    /// Whether the event behind this error should be enqueued for retry.
    /// When in doubt we enqueue (bounded by `max_retries`) rather than drop data.
    pub fn should_enqueue_related_event(error: &EventProcessorError) -> bool {
        match error {
            EventProcessorError::PubkyClientError(err) => match err.as_ref() {
                PubkyClientError::NotInitialized
                | PubkyClientError::TooManyRequests429 { .. }
                | PubkyClientError::ServerError5xx { .. }
                | PubkyClientError::RequestFailed { .. }
                | PubkyClientError::PkarrFailed(..) => true,

                PubkyClientError::NotFound404 { .. }
                | PubkyClientError::AuthenticationFailed(..)
                | PubkyClientError::BuildFailed(..)
                | PubkyClientError::ParseFailed(..) => false,
            },

            EventProcessorError::InvalidEventLine(_)
            | EventProcessorError::SkipIndexing
            | EventProcessorError::SpecValidation(..)
            | EventProcessorError::HsEventsStreamRateLimitExhausted
            | EventProcessorError::UserIdMismatch { .. } => false,

            _ => true,
        }
    }

    pub async fn queue_missing_dep(
        &self,
        event: &Event,
        origin_homeserver_id: &str,
    ) -> Result<(), EventProcessorError> {
        self.enqueue(
            event,
            self.initial.missing_dep_ms,
            "missing dependency",
            origin_homeserver_id,
        )
        .await
    }

    pub async fn queue_transient(
        &self,
        event: &Event,
        origin_homeserver_id: &str,
    ) -> Result<(), EventProcessorError> {
        self.enqueue(
            event,
            self.initial.transient_ms,
            "client error",
            origin_homeserver_id,
        )
        .await
    }

    async fn enqueue(
        &self,
        event: &Event,
        initial_backoff_ms: i64,
        reason: &str,
        origin_homeserver_id: &str,
    ) -> Result<(), EventProcessorError> {
        let next_retry_at = Utc::now().timestamp_millis() + initial_backoff_ms;
        let retry_event = RetryEvent::new(event, next_retry_at, origin_homeserver_id);

        // New EventRetries for the same URI will reset the retry_count
        // The HS state changed since the earlier event, so we disregard previous retry attempts
        self.store.put(&retry_event).await?;
        warn!("Queued event for retry ({}): {}", reason, event.uri);
        Ok(())
    }
}
