use std::cmp::min;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use nexus_common::config::EventRetryConfig;
use nexus_common::models::event::{Event, EventProcessorError, ParseResult};
use nexus_common::WatcherConfig;
use tokio::sync::watch::Receiver;
use tracing::{debug, info, warn};

use super::store::{RedisRetryStore, RetryStore};
use super::RetryScheduler;
use super::{RetryEvent, RetryEventIndexKey};
use crate::events::{DefaultEventHandler, EventHandler, Moderation};
use crate::service::indexer::TEventProcessor;

/// Maximum number of retry events to fetch per batch to avoid memory spikes
const RETRY_BATCH_SIZE: usize = 100;

/// Processor for retrying events that failed due to missing dependencies
pub struct RetryProcessor {
    pub files_path: PathBuf,
    pub event_handler: Arc<dyn EventHandler>,
    pub shutdown_rx: Receiver<bool>,
    pub config: EventRetryConfig,
    /// Persistence backend for retry events. Production wiring uses
    /// [`RedisRetryStore`]; tests swap in an in-memory store for isolation.
    pub store: Arc<dyn RetryStore>,
}

#[async_trait::async_trait]
impl TEventProcessor for RetryProcessor {
    fn files_path(&self) -> &PathBuf {
        &self.files_path
    }

    fn event_handler(&self) -> &Arc<dyn EventHandler> {
        &self.event_handler
    }

    fn instance_name(&self) -> String {
        "RetryProcessor".to_string()
    }

    fn retry_scheduler(&self) -> Option<&Arc<RetryScheduler>> {
        None
    }

    async fn run_internal(self: Arc<Self>) -> Result<(), EventProcessorError> {
        let now = Utc::now().timestamp_millis();

        loop {
            let events = self.fetch_ready_events(now).await?;

            if events.is_empty() {
                debug!("No more events ready for retry");
                return Ok(());
            }

            info!("Processing batch of {} retry events", events.len());

            for (index_key, retry_event) in events {
                if *self.shutdown_rx.borrow() {
                    debug!("Shutdown detected; exiting retry processing loop");
                    return Ok(());
                }

                self.process_retry_event(&index_key, retry_event).await?;
            }
        }
    }
}

impl RetryProcessor {
    pub fn new(config: &WatcherConfig, shutdown_rx: Receiver<bool>) -> Self {
        let moderation = Moderation::from_config(config);
        let store: Arc<dyn RetryStore> = Arc::new(RedisRetryStore::new());
        Self {
            files_path: config.stack.files_path.clone(),
            event_handler: Arc::new(DefaultEventHandler::new(moderation)),
            shutdown_rx,
            config: config.retry.clone(),
            store,
        }
    }

    /// Fetch events from the retry queue that are ready to be retried.
    /// Resolved `(index_key, RetryEvent)` pairs are returned directly by the
    /// store; stale-entry cleanup is the store's responsibility.
    async fn fetch_ready_events(
        &self,
        now: i64,
    ) -> Result<Vec<(RetryEventIndexKey, RetryEvent)>, EventProcessorError> {
        self.store.fetch_ready(now, Some(RETRY_BATCH_SIZE)).await
    }

    /// Process a single retry event
    async fn process_retry_event(
        &self,
        index_key: &RetryEventIndexKey,
        retry_event: RetryEvent,
    ) -> Result<(), EventProcessorError> {
        // Reconstruct the event line and parse the event
        // Event format is "METHOD URI" (e.g., "PUT pubky://...")
        let event_line = format!("{} {}", retry_event.event_type, retry_event.event_uri);

        // Parse the event from the line - if corrupted, remove and continue
        let event = match Event::parse_event(&event_line, self.files_path().clone()) {
            Ok(ParseResult::Parsed(event)) => event,
            Ok(ParseResult::Skipped) | Err(_) => {
                warn!("Corrupted retry entry for key {index_key}, removing: '{event_line}'");
                self.store.remove(index_key).await?;
                return Ok(());
            }
            Ok(ParseResult::UnrecognizedUri { reason, .. }) => {
                warn!("Unrecognized URI in retry entry for key {index_key}, removing: {reason}");
                self.store.remove(index_key).await?;
                return Ok(());
            }
        };

        let ev_uri = &retry_event.event_uri;
        let ev_retry_count = retry_event.retry_count;

        // Call event_handler directly to get the actual error (bypassing handle_event/handle_error)
        let event_handle_res = self.event_handler().handle(&event).await.inspect_err(|e| {
            // In case of error, log it before the error itself is classified and handled
            // Error handling could itself throw an error. We log it here to pre-empt this possibility.
            warn!("Retry event handling failed: {e}");
        });

        match event_handle_res {
            Ok(()) => {
                // Success - event was processed, remove from retry queue
                debug!("Retry successful for event: {ev_uri}");
                self.store.remove(index_key).await?;
            }
            Err(e) if e.is_404() => {
                // Content gone - remove from retry queue
                warn!("Content no longer exists (404) for retry: {ev_uri}");
                self.store.remove(index_key).await?;
            }
            Err(e) if !e.is_retryable() => {
                // Non-retryable error (ParseFailed, etc.) - dead-letter immediately
                warn!("Event {ev_uri} failed with non-retryable error, dead-lettering: {e}");
                self.store.remove(index_key).await?;
            }
            Err(e) if e.is_infrastructure() => {
                // Infrastructure errors (Neo4j/Redis failures) must NOT count against the
                // application-level max_retries limit.  Reschedule with backoff but do NOT
                // increment retry_count, then propagate to stop the current batch.
                self.reschedule(&retry_event, index_key, &e, false).await?;
                return Err(e);
            }
            Err(e) if ev_retry_count >= self.get_max_retries_for_err(&e) => {
                warn!("Event {ev_uri} exceeded max retries ({ev_retry_count}), dead-lettering");
                self.store.remove(index_key).await?;
            }
            Err(e) => {
                // Schedule retry with backoff (increments retry_count)
                self.reschedule(&retry_event, index_key, &e, true).await?;
            }
        }

        Ok(())
    }

    /// Reschedule an event for retry with exponential backoff.
    ///
    /// When `increment_count` is `true` the retry budget is consumed (application-level
    /// errors).  When `false` the counter stays unchanged — used for infrastructure
    /// errors that should not count against the retry limit.
    async fn reschedule(
        &self,
        retry_event: &RetryEvent,
        index_key: &RetryEventIndexKey,
        error: &EventProcessorError,
        increment_count: bool,
    ) -> Result<(), EventProcessorError> {
        let new_retry_count = match increment_count {
            true => retry_event.retry_count + 1,
            false => retry_event.retry_count,
        };

        let initial = match error.is_missing_dependency() {
            true => self.config.initial_missing_dep_backoff_secs,
            false => self.config.initial_backoff_secs,
        };
        let max = match error.is_missing_dependency() {
            true => self.config.max_missing_dep_backoff_secs,
            false => self.config.max_backoff_secs,
        };

        // Calculate backoff based on error type
        // Use retry_count (not new_retry_count) so first retry uses 2^0 * initial = initial
        let backoff_secs = self.calculate_backoff(retry_event.retry_count, initial, max);

        let now = Utc::now().timestamp_millis();
        let next_retry_at = now + (backoff_secs as i64 * 1000);

        let mut updated_event = retry_event.clone();
        updated_event.retry_count = new_retry_count;
        updated_event.next_retry_at = next_retry_at;

        self.store.put(index_key, &updated_event).await?;

        let retry_time =
            DateTime::<Utc>::from_timestamp_millis(next_retry_at).unwrap_or_else(Utc::now);
        info!(
            "Rescheduling {} for {:?} (backoff: {}s, retry_count: {})",
            retry_event.event_uri, retry_time, backoff_secs, new_retry_count
        );

        Ok(())
    }

    /// Calculate exponential backoff
    fn calculate_backoff(&self, retry_count: u32, initial: u64, max: u64) -> u64 {
        let exponential = 2u64
            .checked_pow(retry_count)
            .and_then(|p| initial.checked_mul(p))
            .unwrap_or(max);
        min(exponential, max)
    }

    fn get_max_retries_for_err(&self, e: &EventProcessorError) -> u32 {
        if e.is_missing_dependency() {
            self.config.max_dependency_retries
        } else {
            self.config.max_retries
        }
    }
}
