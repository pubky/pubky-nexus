use std::cmp::min;
use std::sync::Arc;

use crate::errors::EventProcessorError;
use crate::events::{Event, ParseResult};
use chrono::{DateTime, Utc};
use nexus_common::config::EventRetryConfig;
use nexus_common::WatcherConfig;
use tokio::sync::watch::Receiver;
use tracing::{debug, error, info, warn};

use super::store::{RedisRetryStore, RetryStore};
use super::IndexKey;
use super::RetryEvent;
use super::RetryScheduler;
use crate::events::{confirm_flip, is_flip, EventHandler};
use crate::service::indexer::{RunOutcome, TEventProcessor};

/// Maximum number of retry events to fetch per batch to avoid memory spikes
const RETRY_BATCH_SIZE: usize = 100;

/// Processor for retrying events that failed due to missing dependencies
pub struct RetryProcessor {
    pub event_handler: Arc<dyn EventHandler>,
    pub shutdown_rx: Receiver<bool>,
    pub config: EventRetryConfig,
    /// Persistence backend for retry events. Production wiring uses
    /// [`RedisRetryStore`]; tests swap in an in-memory store for isolation.
    pub store: Arc<dyn RetryStore>,
}

#[async_trait::async_trait]
impl TEventProcessor for RetryProcessor {
    fn event_handler(&self) -> &Arc<dyn EventHandler> {
        &self.event_handler
    }

    fn instance_name(&self) -> &'static str {
        "RetryProcessor"
    }

    fn retry_scheduler(&self) -> Option<&Arc<RetryScheduler>> {
        None
    }

    async fn run_internal(self: Arc<Self>) -> Result<RunOutcome, EventProcessorError> {
        let now = Utc::now().timestamp_millis();

        loop {
            let events = self.fetch_ready_events(now).await?;

            if events.is_empty() {
                debug!("No more events ready for retry");
                return Ok(RunOutcome::Completed);
            }

            info!("Processing batch of {} retry events", events.len());

            for (index_key, retry_event) in events {
                if *self.shutdown_rx.borrow() {
                    debug!("Shutdown detected; exiting retry processing loop");
                    return Ok(RunOutcome::Completed);
                }

                self.process_retry_event(&index_key, retry_event).await?;
            }
        }
    }
}

impl RetryProcessor {
    /// Creates a processor retrying events from the Redis queue through `event_handler`
    pub fn new(
        config: &WatcherConfig,
        event_handler: Arc<dyn EventHandler>,
        shutdown_rx: Receiver<bool>,
    ) -> Self {
        let store: Arc<dyn RetryStore> = Arc::new(RedisRetryStore::new());
        Self {
            event_handler,
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

        // Anything but `Parsed` is removed and the batch continues. The fetched
        // RetryEvent deserialized fine, so its nonce is trustworthy and the
        // removal stays conditional: a newer entry for the same URI must survive.
        let event = match Event::parse_event(&event_line) {
            Ok(ParseResult::Parsed(event)) => event,
            Ok(ParseResult::Skipped(reason)) => {
                // Skipped by design (`social/vN`, mute): queued before the parse
                // step started skipping such lines ahead of any fetch.
                debug!(
                    "Retry entry skipped by design ({reason:?}) for key {index_key}, removing: '{event_line}'"
                );
                return self
                    .remove_if_current(index_key, retry_event.nonce, &retry_event.event_uri)
                    .await;
            }
            Ok(ParseResult::UnrecognizedUri { reason, .. }) => {
                warn!("Unrecognized URI in retry entry for key {index_key}, removing: {reason}");
                return self
                    .remove_if_current(index_key, retry_event.nonce, &retry_event.event_uri)
                    .await;
            }
            Err(_) => {
                warn!("Corrupted retry entry for key {index_key}, removing: '{event_line}'");
                return self
                    .remove_if_current(index_key, retry_event.nonce, &retry_event.event_uri)
                    .await;
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
        let Err(error) = event_handle_res else {
            // Success - event was processed, remove from retry queue
            debug!("Retry successful for event: {ev_uri}");
            return self
                .remove_if_current(index_key, retry_event.nonce, ev_uri)
                .await;
        };

        let limit = self.limit_reached(&event, ev_retry_count, &error).await;
        let (e, outcome) = retry_outcome(error, limit);
        match outcome {
            RetryOutcome::Drop => {
                // Not worth retrying (ParseFailed, etc.) - dead-letter immediately. No flip is
                // dead-lettered here: its step runs before the fetch and every failure of it is
                // queued, so a `social/v1` profile PUT that is 404 or invalid is a `v1` user's
                // own edit.
                warn!("Event {ev_uri} threw an error not worth retrying, dead-lettering: {e}");
                self.remove_if_current(index_key, retry_event.nonce, ev_uri)
                    .await?;
            }
            RetryOutcome::RescheduleUncounted => {
                // Errors we should not retry right now (e.g. Neo4j/Redis failures) must NOT count
                // against the application-level max_retries limit.  Reschedule with backoff but do
                // NOT increment retry_count, then propagate to stop the current batch.
                self.reschedule(&retry_event, &e, false).await?;
                return Err(e);
            }
            RetryOutcome::DeadLetter => {
                // A flip has no limit for a failed flip step (`limit_reached`), so a `social/v1`
                // profile PUT reaching it is usually a `v1` user's edit, logged at `warn`; a flip
                // lost to any other error is logged at `error` (`is_flip`).
                if is_flip(&event).await {
                    error!("social/v1 profile PUT {ev_uri} exceeded max retries ({ev_retry_count}), dead-lettering: {e}");
                } else {
                    warn!("Event {ev_uri} exceeded max retries ({ev_retry_count}), dead-lettering: {e}");
                }
                self.remove_if_current(index_key, retry_event.nonce, ev_uri)
                    .await?;
            }
            RetryOutcome::Reschedule => {
                // Schedule retry with backoff (increments retry_count)
                self.reschedule(&retry_event, &e, true).await?;
            }
        }

        Ok(())
    }

    /// Whether `retry_count` attempts have used up the limit `error` gives `event`.
    ///
    /// A flip, the `social/v1` profile PUT of a user at `v0`, has no limit for a failed flip
    /// step (`FlipFailed`): losing it would strand the user at `v0`, so a Neo4j or Redis outage
    /// of any length delays it instead. The gate, a graph read, is asked only once such an
    /// entry reaches its limit. The flip step runs before the profile fetch, so a fetch error
    /// on such a PUT is a `v1` user's edit, which keeps the limit like any other event.
    async fn limit_reached(
        &self,
        event: &Event,
        retry_count: u32,
        error: &EventProcessorError,
    ) -> Result<bool, EventProcessorError> {
        if retry_count < self.max_retries_for(error) {
            return Ok(false);
        }
        let unlimited = error.is_flip_failed() && confirm_flip(event).await?;
        Ok(!unlimited)
    }

    /// Remove the entry for `index_key` only if it still carries `nonce`, i.e.
    /// it is still the event this processor fetched. Logs at debug when the
    /// entry was superseded by a newer event enqueued for the same URI or was
    /// already removed.
    async fn remove_if_current(
        &self,
        index_key: &IndexKey,
        nonce: i64,
        uri: &str,
    ) -> Result<(), EventProcessorError> {
        if !self.store.remove_if(index_key, nonce).await? {
            debug!(
                "Retry entry superseded by a newer event or already removed for this URI, skipping: {uri}"
            );
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
                "Retry entry superseded by a newer event or already removed for this URI, skipping: {}",
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

/// What the retry processor does with an entry whose handling failed with `error`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RetryOutcome {
    /// Removed at once: the error is not worth retrying.
    Drop,
    /// Rescheduled without counting the attempt, and the batch stops: Neo4j or Redis is down.
    RescheduleUncounted,
    /// Removed: the entry used up its limit.
    DeadLetter,
    /// Rescheduled, the attempt counted.
    Reschedule,
}

/// Classifies a failed retry. Pure: the caller reads the limit ([`RetryProcessor::limit_reached`])
/// and does the I/O the outcome names. Returns the error to log and reschedule with, and the
/// outcome.
///
/// A failed limit check, a graph read of the user's epoch, takes the handler's error's place
/// and never removes the entry: dead-lettering a flip because the graph could not say whether
/// it is one would strand its user at `v0`. It is rescheduled uncounted when it stops the
/// batch, counted otherwise.
fn retry_outcome(
    error: EventProcessorError,
    limit: Result<bool, EventProcessorError>,
) -> (EventProcessorError, RetryOutcome) {
    let limit_reached = match limit {
        Ok(limit_reached) => limit_reached,
        Err(gate_error) => {
            let outcome = if gate_error.should_not_retry_now() {
                RetryOutcome::RescheduleUncounted
            } else {
                RetryOutcome::Reschedule
            };
            return (gate_error, outcome);
        }
    };
    let outcome = if !RetryScheduler::should_enqueue_related_event(&error) {
        RetryOutcome::Drop
    } else if error.should_not_retry_now() {
        RetryOutcome::RescheduleUncounted
    } else if limit_reached {
        RetryOutcome::DeadLetter
    } else {
        RetryOutcome::Reschedule
    };
    (error, outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::gate::flip_profile_uri;
    use crate::events::retry::store::InMemoryRetryStore;
    use crate::events::EventType;
    use nexus_common::db::{exec_single_row, queries};
    use nexus_common::models::traits::Collection as _;
    use nexus_common::models::user::{Epoch, UserDetails};
    use nexus_common::types::DynError;
    use nexus_common::utils::test_utils::random_pubky_id;
    use nexus_common::{StackConfig, StackManager};
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

    /// Lost-update clobber scenario end-to-end through the processor: the
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

    /// Handler that fails every event with `error`.
    struct FailingHandler {
        error: EventProcessorError,
    }

    #[async_trait::async_trait]
    impl EventHandler for FailingHandler {
        async fn handle(&self, _event: &Event) -> Result<(), EventProcessorError> {
            Err(self.error.clone())
        }
    }

    /// A flip step only fails for a user at `v0`, so a `FlipFailed` entry whose user is `v1`
    /// in the graph is a stale one: at the limit it is dead-lettered as the profile edit it
    /// then is, at `warn` with the error's text, never at `error`. The in-crate counterpart
    /// of the `v0` tests in `tests/service/retry_processor.rs`, here because only a test in
    /// this crate can read the processor's log lines.
    #[tokio_shared_rt::test(shared)]
    #[tracing_test::traced_test]
    async fn a_failed_flip_step_of_a_v1_user_at_the_limit_is_dead_lettered_at_warn(
    ) -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let user = random_pubky_id();
        UserDetails::from_pubky(user.clone()).put_to_graph().await?;
        Epoch::set_v1_in_graph(&user).await?;

        let flip_uri = flip_profile_uri(&user);
        let index_key = IndexKey::for_uri(&flip_uri);
        let config = EventRetryConfig::default();
        let store: Arc<dyn RetryStore> = Arc::new(InMemoryRetryStore::new());
        store
            .put(&RetryEvent {
                retry_count: config.max_retries,
                event_type: EventType::Put,
                event_uri: flip_uri,
                next_retry_at: Utc::now().timestamp_millis() - 1_000,
                origin_homeserver_id: "test_hs".to_string(),
                nonce: 1,
            })
            .await?;

        let error = EventProcessorError::FlipFailed {
            step: "posts",
            source: Box::new(EventProcessorError::GraphQueryFailed(
                true,
                "connection refused".to_string(),
            )),
        };
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        let processor = Arc::new(RetryProcessor {
            event_handler: Arc::new(FailingHandler {
                error: error.clone(),
            }),
            shutdown_rx,
            config,
            store: store.clone(),
        });

        processor.run_internal().await?;

        assert!(
            store.get(&index_key).await?.is_none(),
            "a v1 user's entry is dead-lettered at the limit"
        );
        assert_eq!(Epoch::get_from_graph(&user).await?, Epoch::V1);
        let text = error.to_string();
        logs_assert(|lines| {
            let dead_letter: Vec<_> = lines
                .iter()
                .filter(|line| line.contains("dead-lettering"))
                .collect();
            match dead_letter.as_slice() {
                [line] if line.contains(" WARN ") && line.contains(&text) => Ok(()),
                other => Err(format!(
                    "expected one WARN dead-letter line carrying `{text}`, got {other:?}"
                )),
            }
        });

        exec_single_row(queries::del::delete_user(&user)).await?;
        Ok(())
    }

    /// The classification of a failed retry, for the errors that decide it. A failed limit
    /// check never removes the entry, whatever its class; a flip's limit is the check's to
    /// lift (`Ok(false)`), so `FlipFailed` dead-letters only at a `v1` user's limit.
    #[test]
    fn retry_outcome_classifies_every_error_and_never_removes_on_a_failed_limit_check() {
        use RetryOutcome::*;
        let graph_down =
            || EventProcessorError::GraphQueryFailed(true, "connection refused".into());
        let graph_bad_row =
            || EventProcessorError::GraphQueryFailed(false, "deserialization failed".into());
        let anomaly = || EventProcessorError::DataAnomaly("keyset scan cannot advance".into());
        let flip_failed = || EventProcessorError::FlipFailed {
            step: "posts",
            source: Box::new(graph_bad_row()),
        };
        let not_found = || EventProcessorError::client_error_404("gone".into());
        let missing = || EventProcessorError::missing_dependencies(vec![URI.to_string()]);

        type Case = (
            &'static str,
            EventProcessorError,
            Result<bool, EventProcessorError>,
            RetryOutcome,
        );
        // (case, handler error, limit check, outcome)
        let cases: Vec<Case> = vec![
            (
                "flip, graph down at the check",
                flip_failed(),
                Err(graph_down()),
                RescheduleUncounted,
            ),
            (
                "flip, bad row at the check",
                flip_failed(),
                Err(graph_bad_row()),
                Reschedule,
            ),
            (
                "flip, anomaly at the check",
                flip_failed(),
                Err(anomaly()),
                Reschedule,
            ),
            (
                "flip of a v1 user at its limit",
                flip_failed(),
                Ok(true),
                DeadLetter,
            ),
            (
                "flip under its limit or of a v0 user",
                flip_failed(),
                Ok(false),
                Reschedule,
            ),
            ("404 under its limit", not_found(), Ok(false), Drop),
            ("404 at its limit", not_found(), Ok(true), Drop),
            (
                "graph down at the limit",
                graph_down(),
                Ok(true),
                RescheduleUncounted,
            ),
            (
                "missing dependency at its limit",
                missing(),
                Ok(true),
                DeadLetter,
            ),
            (
                "missing dependency under its limit",
                missing(),
                Ok(false),
                Reschedule,
            ),
        ];
        for (case, error, limit, expected) in cases {
            // A failed limit check takes the handler's error's place
            let expected_error = match &limit {
                Err(check_error) => check_error.to_string(),
                Ok(_) => error.to_string(),
            };
            let (returned, outcome) = retry_outcome(error, limit);
            assert_eq!(outcome, expected, "{case}");
            assert_eq!(
                returned.to_string(),
                expected_error,
                "{case}: the error kept"
            );
        }
    }
}
