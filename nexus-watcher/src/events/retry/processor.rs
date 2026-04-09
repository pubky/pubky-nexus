use std::cmp::min;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use tokio::sync::watch::Receiver;
use tracing::{debug, info, warn};

use nexus_common::models::event::{Event, EventProcessorError, EventType};
use nexus_common::WatcherConfig;

use super::scheduler::RetryScheduler;
use super::store::{RedisRetryStore, RetryStore};
use super::{InitialBackoff, RetryEvent};
use crate::events::{DefaultEventHandler, EventHandler, Moderation, TModeration};
use crate::service::indexer::TEventProcessor;

/// Maximum number of retry events to fetch per batch to avoid memory spikes
const RETRY_BATCH_SIZE: usize = 100;

/// Processor for retrying events that failed due to missing dependencies
pub struct RetryProcessor {
    pub files_path: PathBuf,
    pub event_handler: Arc<dyn EventHandler>,
    pub shutdown_rx: Receiver<bool>,
    pub config: RetryProcessorConfig,
    /// Persistence backend for retry events. Production wiring uses
    /// [`RedisRetryStore`]; tests swap in an in-memory store for isolation.
    pub store: Arc<dyn RetryStore>,
    /// Provided only to satisfy [`TEventProcessor::retry_scheduler`]. The retry
    /// processor bypasses [`TEventProcessor::handle_error`] (it calls the event
    /// handler directly and reschedules via [`Self::schedule_retry`]), so the
    /// scheduler is never actually invoked from this processor.
    pub retry_scheduler: Arc<RetryScheduler>,
}

/// Configuration for the retry processor
#[derive(Debug, Clone)]
pub struct RetryProcessorConfig {
    /// Max retries for transient errors
    pub max_retries: u32,
    /// Max retries for missing dependency errors
    pub max_dependency_retries: u32,
    /// Initial backoff for transient errors (seconds)
    pub initial_backoff_secs: u64,
    /// Max backoff for transient errors (seconds)
    pub max_backoff_secs: u64,
    /// Initial backoff for missing dependency (seconds)
    pub initial_missing_dep_backoff_secs: u64,
    /// Max backoff for missing dependency (seconds)
    pub max_missing_dep_backoff_secs: u64,
}

impl RetryProcessorConfig {
    pub fn from_watcher_config(config: &WatcherConfig) -> Self {
        let r = &config.retry;
        Self {
            max_retries: r.max_retries,
            max_dependency_retries: r.max_dependency_retries,
            initial_backoff_secs: r.initial_backoff_secs,
            max_backoff_secs: r.max_backoff_secs,
            initial_missing_dep_backoff_secs: r.initial_missing_dep_backoff_secs,
            max_missing_dep_backoff_secs: r.max_missing_dep_backoff_secs,
        }
    }
}

#[async_trait::async_trait]
impl TEventProcessor for RetryProcessor {
    fn files_path(&self) -> &PathBuf {
        &self.files_path
    }

    fn moderation(&self) -> &Arc<dyn TModeration> {
        self.event_handler.moderation()
    }

    fn event_handler(&self) -> &Arc<dyn EventHandler> {
        &self.event_handler
    }

    fn instance_name(&self) -> String {
        "RetryProcessor".to_string()
    }

    fn retry_scheduler(&self) -> &Arc<RetryScheduler> {
        &self.retry_scheduler
    }

    async fn run_internal(self: Arc<Self>) -> Result<(), EventProcessorError> {
        let now = Utc::now().timestamp_millis();

        loop {
            if *self.shutdown_rx.borrow() {
                debug!("Shutdown detected; exiting retry processing loop");
                return Ok(());
            }

            let events = self.fetch_ready_events(now).await?;

            if events.is_empty() {
                debug!("No more events ready for retry");
                return Ok(());
            }

            info!("Processing batch of {} retry events", events.len());

            for (resource_key, retry_event) in events {
                if *self.shutdown_rx.borrow() {
                    debug!("Shutdown detected; exiting retry processing loop");
                    return Ok(());
                }

                self.process_retry_event(&resource_key, retry_event).await?;
            }
        }
    }
}

impl RetryProcessor {
    pub fn new(config: &WatcherConfig, shutdown_rx: Receiver<bool>) -> Self {
        let moderation = Moderation::from_config(config);
        let store: Arc<dyn RetryStore> = Arc::new(RedisRetryStore::new());
        let retry_scheduler = Arc::new(RetryScheduler::new(
            store.clone(),
            InitialBackoff::from_config(config),
        ));
        Self {
            files_path: config.stack.files_path.clone(),
            event_handler: Arc::new(DefaultEventHandler::new(moderation)),
            shutdown_rx,
            config: RetryProcessorConfig::from_watcher_config(config),
            store,
            retry_scheduler,
        }
    }

    /// Fetch events from the retry queue that are ready to be retried.
    /// Resolved `(resource_key, RetryEvent)` pairs are returned directly by the
    /// store; stale-entry cleanup is the store's responsibility.
    async fn fetch_ready_events(
        &self,
        now: i64,
    ) -> Result<Vec<(String, RetryEvent)>, EventProcessorError> {
        self.store.fetch_ready(now, Some(RETRY_BATCH_SIZE)).await
    }

    /// Process a single retry event
    async fn process_retry_event(
        &self,
        resource_key: &str,
        retry_event: RetryEvent,
    ) -> Result<(), EventProcessorError> {
        // Reconstruct the event line and parse the event
        // Event format is "METHOD URI" (e.g., "PUT pubky://...")
        let event_type_str = match retry_event.event_type {
            EventType::Put => "PUT",
            EventType::Del => "DEL",
        };
        let event_line = format!("{} {}", event_type_str, retry_event.event_uri);

        // Parse the event from the line - if corrupted, remove and continue
        let event = match Event::parse_event(&event_line, self.files_path().clone()) {
            Ok(Some(event)) => event,
            Ok(None) | Err(_) => {
                warn!(
                    "Corrupted retry entry for key {}, removing: {}",
                    resource_key, event_line
                );
                self.store.remove(resource_key).await?;
                return Ok(());
            }
        };

        // Call event_handler directly to get the actual error (bypassing handle_event/handle_error)
        match self.event_handler().handle(&event).await {
            Ok(()) => {
                // Success - event was processed, remove from retry queue
                debug!("Retry successful for event: {}", retry_event.event_uri);
                self.store.remove(resource_key).await?;
            }
            Err(e) if e.is_404() => {
                // Content gone - remove from retry queue
                warn!(
                    "Content no longer exists (404) for retry: {}",
                    retry_event.event_uri
                );
                self.store.remove(resource_key).await?;
            }
            Err(e) if !e.is_retryable() => {
                // Non-retryable error (ParseFailed, etc.) - dead-letter immediately
                warn!(
                    "Event {} failed with non-retryable error, dead-lettering: {}",
                    retry_event.event_uri, e
                );
                self.store.remove(resource_key).await?;
            }
            Err(e) => {
                // Check if we've exceeded max retries based on current error type
                let max_retries = if e.is_missing_dependency() {
                    self.config.max_dependency_retries
                } else {
                    self.config.max_retries
                };

                if retry_event.retry_count >= max_retries {
                    warn!(
                        "Event {} exceeded max retries ({}) - dead-lettering",
                        retry_event.event_uri, retry_event.retry_count
                    );
                    // Remove from retry queue (dead-lettered)
                    self.store.remove(resource_key).await?;
                    return Ok(());
                }

                // Schedule retry with backoff
                self.schedule_retry(&retry_event, resource_key, &e).await?;

                // Propagate infrastructure errors to stop the batch
                if e.is_infrastructure() {
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    /// Schedule an event for retry with exponential backoff
    async fn schedule_retry(
        &self,
        retry_event: &RetryEvent,
        resource_key: &str,
        error: &EventProcessorError,
    ) -> Result<(), EventProcessorError> {
        let new_retry_count = retry_event.retry_count + 1;
        let now = Utc::now().timestamp_millis();

        // Calculate backoff based on error type
        // Use retry_count (not new_retry_count) so first retry uses 2^0 * initial = initial
        let backoff_secs = if error.is_missing_dependency() {
            // Missing dependency backoff (longer initial, higher ceiling)
            self.calculate_backoff(
                retry_event.retry_count,
                self.config.initial_missing_dep_backoff_secs,
                self.config.max_missing_dep_backoff_secs,
            )
        } else {
            // Transient error backoff
            self.calculate_backoff(
                retry_event.retry_count,
                self.config.initial_backoff_secs,
                self.config.max_backoff_secs,
            )
        };

        let next_retry_at = now + (backoff_secs as i64 * 1000);

        // Update the retry event
        let mut updated_event = retry_event.clone();
        updated_event.retry_count = new_retry_count;
        updated_event.next_retry_at = next_retry_at;

        // Update in index
        self.store.put(resource_key, &updated_event).await?;

        let retry_time =
            DateTime::<Utc>::from_timestamp_millis(next_retry_at).unwrap_or_else(Utc::now);
        info!(
            "Scheduled retry {} for event {} at {:?} (backoff: {}s)",
            new_retry_count, retry_event.event_uri, retry_time, backoff_secs
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
}
