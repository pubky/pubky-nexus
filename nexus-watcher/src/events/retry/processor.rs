use std::cmp::min;
use std::path::PathBuf;
use std::sync::Arc;

use crate::errors::EventProcessorError;
use crate::events::{Event, ParseResult};
use chrono::{DateTime, Utc};
use nexus_common::config::EventRetryConfig;
use nexus_common::WatcherConfig;
use tokio::sync::watch::Receiver;
use tracing::{debug, info, warn};

use super::store::{RedisRetryStore, RetryStore};
use super::IndexKey;
use super::RetryEvent;
use super::RetryScheduler;
use crate::events::{DefaultEventHandler, EventHandler};
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
        let store: Arc<dyn RetryStore> = Arc::new(RedisRetryStore::new());
        Self {
            files_path: config.stack.files_path.clone(),
            event_handler: Arc::new(DefaultEventHandler::from_config(config)),
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
    ) -> Result<Vec<(IndexKey, RetryEvent)>, EventProcessorError> {
        self.store.fetch_ready(now, Some(RETRY_BATCH_SIZE)).await
    }

    /// Process a single retry event
    async fn process_retry_event(
        &self,
        index_key: &IndexKey,
        retry_event: RetryEvent,
    ) -> Result<(), EventProcessorError> {
        // Reconstruct the event line and parse the event
        // Event format is "METHOD URI" (e.g., "PUT pubky://...")
        let event_line = format!("{} {}", retry_event.event_type, retry_event.event_uri);

        // Parse the event from the line - if corrupted, remove and continue.
        // The fetched RetryEvent deserialized fine (only the reconstructed event
        // line failed to parse), so its nonce is trustworthy and the cleanup can
        // be conditional too: a newer entry for the same URI must survive.
        let event = match Event::parse_event(&event_line, self.files_path().clone()) {
            Ok(ParseResult::Parsed(event)) => event,
            Ok(ParseResult::Skipped) | Err(_) => {
                warn!("Corrupted retry entry for key {index_key}, removing: '{event_line}'");
                self.remove_if_current(index_key, retry_event.nonce, &retry_event.event_uri)
                    .await?;
                return Ok(());
            }
            Ok(ParseResult::UnrecognizedUri { reason, .. }) => {
                warn!("Unrecognized URI in retry entry for key {index_key}, removing: {reason}");
                self.remove_if_current(index_key, retry_event.nonce, &retry_event.event_uri)
                    .await?;
                return Ok(());
            }
        };

        let ev_uri = &retry_event.event_uri;
        let ev_retry_count = retry_event.retry_count;

        // In principle, it's possible to check if `origin_homeserver_id` is blacklisted before
        // handling the event. A retry entry may have been queued before that HS got blacklisted.
        // Retrying those pre-existing events is acceptable for now. Newly discovered events from a
        // blacklisted HS are blocked before they can be enqueued.
        //
        // Call event_handler directly to get the actual error (bypassing handle_event/handle_error)
        let event_handle_res = self.event_handler().handle(&event).await.inspect_err(|e| {
            // In case of error, log it before the error itself is classified and handled
            // Error handling could itself throw an error. We log it here to pre-empt this possibility.
            warn!("Retry event handling failed: {e}");
        });

        // Removals and reschedules below are conditional on the nonce of the event
        // we fetched: a live processor may have overwritten the hash(URI)-keyed
        // entry with a newer event (fresh nonce) while this retry was in flight,
        // and that newer entry must not be removed or clobbered.
        match event_handle_res {
            Ok(()) => {
                // Success - event was processed, remove from retry queue
                debug!("Retry successful for event: {ev_uri}");
                self.remove_if_current(index_key, retry_event.nonce, ev_uri)
                    .await?;
            }
            Err(e) if !RetryScheduler::should_enqueue_related_event(&e) => {
                // Not worth retrying (ParseFailed, etc.) - dead-letter immediately
                warn!("Event {ev_uri} threw an error not worth retrying, dead-lettering: {e}");
                self.remove_if_current(index_key, retry_event.nonce, ev_uri)
                    .await?;
            }
            Err(e) if e.should_not_retry_now() => {
                // Errors we should not retry right now (e.g. Neo4j/Redis failures) must NOT count
                // against the application-level max_retries limit.  Reschedule with backoff but do
                // NOT increment retry_count, then propagate to stop the current batch.
                self.reschedule(&retry_event, &e, false).await?;
                return Err(e);
            }
            Err(e) if ev_retry_count >= self.max_retries_for(&e) => {
                warn!("Event {ev_uri} exceeded max retries ({ev_retry_count}), dead-lettering");
                self.remove_if_current(index_key, retry_event.nonce, ev_uri)
                    .await?;
            }
            Err(e) => {
                // Schedule retry with backoff (increments retry_count)
                self.reschedule(&retry_event, &e, true).await?;
            }
        }

        Ok(())
    }

    /// Remove the entry for `index_key` only if it still carries `nonce`, i.e.
    /// it is still the event this processor fetched. Logs at debug when the
    /// entry was superseded by a newer event enqueued for the same URI.
    async fn remove_if_current(
        &self,
        index_key: &IndexKey,
        nonce: i64,
        uri: &str,
    ) -> Result<(), EventProcessorError> {
        if !self.store.remove_if(index_key, nonce).await? {
            debug!("Retry entry superseded by a newer event for this URI, leaving it: {uri}");
        }
        Ok(())
    }

    fn max_retries_for(&self, error: &EventProcessorError) -> u32 {
        if error.is_missing_dependency() {
            self.config.max_dependency_retries
        } else {
            self.config.max_retries
        }
    }

    fn backoff_params_for(&self, error: &EventProcessorError) -> (u64, u64) {
        if error.is_missing_dependency() {
            (
                self.config.initial_missing_dep_backoff_secs,
                self.config.max_missing_dep_backoff_secs,
            )
        } else {
            (
                self.config.initial_backoff_secs,
                self.config.max_backoff_secs,
            )
        }
    }

    /// Reschedule an event for retry with exponential backoff.
    ///
    /// When `increment_count` is `true` the retry budget is consumed (application-level
    /// errors).  When `false` the counter stays unchanged — used for errors that
    /// should not be retried right now, which should not count against the retry limit.
    async fn reschedule(
        &self,
        retry_event: &RetryEvent,
        error: &EventProcessorError,
        increment_count: bool,
    ) -> Result<(), EventProcessorError> {
        let new_retry_count = match increment_count {
            true => retry_event.retry_count + 1,
            false => retry_event.retry_count,
        };

        let (initial, max) = self.backoff_params_for(error);
        // Use retry_count (not new_retry_count) so first retry uses 2^0 * initial = initial
        let backoff_secs = calculate_backoff(retry_event.retry_count, initial, max);

        let now = Utc::now().timestamp_millis();
        let next_retry_at = now + (backoff_secs as i64 * 1000);

        let mut updated_event = retry_event.clone();
        updated_event.retry_count = new_retry_count;
        updated_event.next_retry_at = next_retry_at;

        // The clone keeps the nonce: a reschedule updates the same logical event.
        // The conditional put refuses to act if a newer event (fresh nonce) has
        // replaced the entry for this URI while the retry was in flight.
        if !self.store.put_if(&updated_event, retry_event.nonce).await? {
            debug!(
                "Retry entry superseded by a newer event for this URI, leaving it: {}",
                retry_event.event_uri
            );
            return Ok(());
        }

        let retry_time =
            DateTime::<Utc>::from_timestamp_millis(next_retry_at).unwrap_or_else(Utc::now);
        info!(
            "Rescheduling {} for {:?} (backoff: {}s, retry_count: {})",
            retry_event.event_uri, retry_time, backoff_secs, new_retry_count
        );

        Ok(())
    }
}

/// Calculate exponential backoff
fn calculate_backoff(retry_count: u32, initial: u64, max: u64) -> u64 {
    let exponential = 2u64
        .checked_pow(retry_count)
        .and_then(|p| initial.checked_mul(p))
        .unwrap_or(max);
    min(exponential, max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::retry::store::InMemoryRetryStore;
    use crate::events::EventType;
    use tokio::sync::watch;

    /// Valid post URI (z32 user id + pubky.app post path) so
    /// `Event::parse_event` yields `ParseResult::Parsed` without any stack.
    const URI: &str =
        "pubky://uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko/pub/pubky.app/posts/clobberpst";

    /// Handler that simulates a live event processor enqueuing a NEW retry
    /// event (fresh nonce) for the same URI while the fetched retry is still
    /// in flight, i.e. after fetch_ready but before the success-path removal.
    struct EnqueueNewerEventHandler {
        store: Arc<dyn RetryStore>,
        newer: RetryEvent,
    }

    #[async_trait::async_trait]
    impl EventHandler for EnqueueNewerEventHandler {
        async fn handle(&self, _event: &Event) -> Result<(), EventProcessorError> {
            self.store.put(&self.newer).await?;
            Ok(())
        }
    }

    /// The #963 clobber scenario end-to-end through the processor: the
    /// success-path removal of a fetched event must not delete the newer
    /// entry that replaced it mid-flight.
    #[tokio::test]
    async fn success_path_removal_does_not_clobber_event_enqueued_mid_flight() {
        let store: Arc<dyn RetryStore> = Arc::new(InMemoryRetryStore::new());
        let index_key = IndexKey::for_uri(URI);
        let now = Utc::now().timestamp_millis();

        // E1: a PUT already in the queue, ready for retry; the processor
        // fetches and handles it.
        let e1 = RetryEvent {
            retry_count: 0,
            event_type: EventType::Put,
            event_uri: URI.to_string(),
            next_retry_at: now - 1_000,
            origin_homeserver_id: "test_hs".to_string(),
            nonce: 100,
        };
        store.put(&e1).await.unwrap();

        // E2: a DEL for the same URI, enqueued by the handler mid-flight with
        // a fresh nonce. Scheduled in the future so the processor's next
        // fetch_ready pass leaves it alone.
        let e2 = RetryEvent {
            event_type: EventType::Del,
            next_retry_at: now + 60_000,
            nonce: 200,
            ..e1.clone()
        };

        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        let processor = Arc::new(RetryProcessor {
            files_path: PathBuf::from("/tmp/test"),
            event_handler: Arc::new(EnqueueNewerEventHandler {
                store: store.clone(),
                newer: e2,
            }),
            shutdown_rx,
            config: EventRetryConfig::default(),
            store: store.clone(),
        });

        processor
            .run_internal()
            .await
            .expect("processing must succeed");

        // E1's handle() returned Ok(()), so the processor took the success
        // path; its conditional removal must have been a no-op and E2 (the
        // newer entry) must survive.
        let stored = store
            .get(&index_key)
            .await
            .unwrap()
            .expect("newer entry must survive the stale success-path removal");
        assert_eq!(stored.nonce, 200, "stored entry must be E2, not E1");
        assert_eq!(stored.event_type, EventType::Del);
        assert_eq!(stored.next_retry_at, now + 60_000);
    }
}
