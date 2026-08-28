use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, LazyLock},
    time::Duration,
};

use crate::errors::{BatchProcessingError, EventProcessorError};
use crate::events::Event;
use futures::StreamExt;
use nexus_common::db::PubkyConnector;
use nexus_common::models::homeserver::HsBlacklist;
use nexus_common::models::user::UserHsCursor;
use opentelemetry::metrics::Counter;
use opentelemetry::{global, KeyValue};
use pubky::errors::RequestError;
use pubky::{Event as StreamEvent, EventCursor, PublicKey};
use pubky_app_specs::PubkyId;
use tokio::sync::watch::Receiver;
use tracing::{debug, error, info, warn};

use super::TEventProcessor;
use crate::events::retry::RetryScheduler;
use crate::events::EventHandler;
use crate::service::runner::UserNotFoundBackoff;
use crate::service::user_hs_resolver;

const FETCH_EVENTS_429_BACKOFF_SECS: [u64; 3] = [1, 2, 3];

/// Max users a single `/events-stream` request can carry (SDK-enforced).
pub const MAX_BATCH_USERS: usize = 50;

/// Counter for per-user stream events an External HS returned at or below the
/// ordering floor — a replay of already-indexed data, or a regression against an
/// earlier event in the same batch. The whole batch is rejected before any handler
/// runs. Labelled by `hs_id` only to avoid per-user metric cardinality.
static OUT_OF_ORDER_CURSOR_EXTERNAL_HS: LazyLock<Counter<u64>> = LazyLock::new(|| {
    global::meter(super::METER_NAME)
        .u64_counter("watcher.external_hs.cursor.out_of_order")
        .with_description("Per-user stream events a homeserver returned out of cursor order")
        .build()
});

#[async_trait::async_trait]
pub trait KeyBasedEventSource: Send + Sync + 'static {
    /// Fetch events for a batch of users from a homeserver's `/events-stream` endpoint.
    ///
    /// - `users` must be non-empty and contain at most 50 entries (SDK-enforced limit).
    /// - Each user's `Option<EventCursor>` is their individual resume position (`None` = from beginning).
    /// - `limit` caps the total events returned across all users in this
    ///   individual `/events-stream` request.
    async fn fetch_events(
        &self,
        hs_pk: &PublicKey,
        users: &[(&PublicKey, Option<EventCursor>)],
        limit: u16,
    ) -> Result<Vec<StreamEvent>, EventProcessorError>;
}

pub struct PubkyKeyBasedEventSource;

#[async_trait::async_trait]
impl KeyBasedEventSource for PubkyKeyBasedEventSource {
    async fn fetch_events(
        &self,
        hs_pk: &PublicKey,
        users: &[(&PublicKey, Option<EventCursor>)],
        limit: u16,
    ) -> Result<Vec<StreamEvent>, EventProcessorError> {
        let pubky = PubkyConnector::get()?;

        // We are building the stream without the live flag, so it performs an HTTP GET and closes.
        // See rustdoc of EventStreamBuilder::live()
        let mut stream = pubky
            .event_stream_for(hs_pk)
            .add_users(users.iter().copied())?
            .limit(limit)
            .path("/pub/")
            .subscribe()
            .await
            .map_err(|error| match error {
                pubky::Error::Request(RequestError::Transport(error)) => {
                    EventProcessorError::hs_transport_failed(error)
                }
                error => error.into(),
            })
            .inspect_err(|e| error!(error = ?e, "Failed to subscribe to event stream"))?;

        // The HS is asked for at most `limit` events, but a misbehaving one could return more
        let limit = limit as usize;
        let mut events = Vec::with_capacity(limit);
        while let Some(result) = stream.next().await {
            // Read at most `limit` events. If the stream still has more, log an error and drop the rest.
            if events.len() >= limit {
                error!(
                    %hs_pk,
                    user_count = users.len(),
                    limit,
                    "Event stream returned more than the requested limit; ignoring the excess"
                );
                break;
            }

            // Pubky uses SSE framing regardless of EventStreamBuilder::live().
            // Failures after response headers are emitted as stream item errors.
            let event = result.map_err(EventProcessorError::hs_transport_failed)?;
            events.push(event);
        }

        Ok(events)
    }
}

/// Result of processing a single user's events within a batch.
pub struct UserBatchResult {
    pub user_pk: PublicKey,
    pub latest_cursor: Option<u64>,
    pub result: Result<(), EventProcessorError>,
}

/// Outcome of [`KeyBasedEventProcessor::fetch_events_with_404_recovery`].
#[derive(Debug)]
pub struct BatchFetchEvents {
    /// Events fetched from the healthy (sub-)batches, in fetch order.
    pub events: Vec<StreamEvent>,
    /// Users whose fetch returned 404 and were recorded into
    /// [`UserNotFoundBackoff`]. Callers must not record success for these users.
    pub backed_off_users: Vec<PublicKey>,
}

/// Event processor for third-party (external) HSs, where the user-specific `/events-stream`
/// endpoint is used
pub struct KeyBasedEventProcessor {
    /// The HS endpoint this processor fetches events from
    pub homeserver_id: PubkyId,

    /// Max events requested from the homeserver for each `/events-stream`
    /// request. Recovery sub-batches reuse this limit even for one user.
    pub limit: u16,

    pub event_handler: Arc<dyn EventHandler>,
    pub event_source: Arc<dyn KeyBasedEventSource>,
    pub user_not_found_backoff: Arc<UserNotFoundBackoff>,

    /// HS PKs that should not be indexed. Defense-in-depth: the runner already
    /// excludes these from `pre_run`, but the processor refuses to run for a
    /// blacklisted HS too.
    pub hs_blacklist: HsBlacklist,

    /// Scheduler used to enqueue failed events onto the retry queue
    pub retry_scheduler: Arc<RetryScheduler>,

    pub shutdown_rx: Receiver<bool>,
}

#[async_trait::async_trait]
impl TEventProcessor for KeyBasedEventProcessor {
    fn event_handler(&self) -> &Arc<dyn EventHandler> {
        &self.event_handler
    }

    fn instance_name(&self) -> &'static str {
        "KeyBasedEventProcessor"
    }

    fn retry_scheduler(&self) -> Option<&Arc<RetryScheduler>> {
        Some(&self.retry_scheduler)
    }

    fn homeserver_id(&self) -> Option<&str> {
        Some(self.homeserver_id.as_ref())
    }

    fn custom_timeout(&self) -> Option<Duration> {
        Some(Duration::from_mins(8))
    }

    async fn run_internal(self: Arc<Self>) -> Result<(), EventProcessorError> {
        let hs_id = self.homeserver_id.to_string();

        // Blacklisted HSs must never be indexed. The runner already excludes
        // them from `pre_run`, so reaching here is unexpected.
        if self.hs_blacklist.is_blacklisted(&hs_id) {
            error!(action = "abort_hs", "Refusing to process blacklisted HS");
            return Err(EventProcessorError::HsBlacklisted { hs_id });
        }

        let hs_pk = self.homeserver_id.to_public_key();

        let users = self
            .resolve_users_with_cursors()
            .await
            .inspect_err(|e| error!(error = ?e, "Failed to resolve users"))?;

        if users.is_empty() {
            debug!("No users, skipping");
            return Ok(());
        }

        info!(user_count = users.len(), "Found users");

        // Filter out users whose event fetch previously returned 404; they are
        // skipped for an increasing number of runs (see `UserNotFoundBackoff`).
        let mut active_users = Vec::with_capacity(users.len());
        for (user_pk, cursor) in users {
            if *self.shutdown_rx.borrow() {
                debug!("Shutdown detected; stopping user filtering");
                break;
            }
            if self.user_not_found_backoff.consume_skip(&user_pk).await {
                debug!(user_id = %user_pk.z32(), action = "skip_user", "Skipping user due to 404 backoff");
                continue;
            }
            active_users.push((user_pk, cursor));
        }

        // Chunk into batches of up to 50 users; one /events-stream call per chunk.
        for chunk in active_users.chunks(MAX_BATCH_USERS) {
            if *self.shutdown_rx.borrow() {
                debug!("Shutdown detected; stopping user batch iteration");
                break;
            }

            if let Err(err) = self.process_users(&hs_pk, chunk).await {
                if err.should_not_retry_now() {
                    error!(?err, "Fatal error during batch processing; aborting HS run");
                    return Err(err);
                }
                error!(
                    ?err,
                    "Recoverable error during batch processing; continuing with next batch"
                );
            }
        }

        Ok(())
    }
}

impl KeyBasedEventProcessor {
    /// Resolves monitored users on this homeserver and reads their cursors from Redis.
    #[tracing::instrument(name = "dx.users.resolve", skip_all)]
    async fn resolve_users_with_cursors(
        &self,
    ) -> Result<Vec<(PublicKey, EventCursor)>, EventProcessorError> {
        let hs_id: &str = self.homeserver_id.as_ref();
        let user_ids = user_hs_resolver::get_user_ids_by_homeserver(hs_id).await?;
        debug!(user_count = user_ids.len(), "Resolved users");

        let mut valid_users: Vec<(PublicKey, &str)> = Vec::with_capacity(user_ids.len());
        for user_id in &user_ids {
            let Ok(user_pk) = user_id.parse::<PublicKey>() else {
                warn!(%user_id, "Invalid user public key, skipping");
                continue;
            };
            valid_users.push((user_pk, user_id.as_str()));
        }

        let user_id_strs: Vec<&str> = valid_users.iter().map(|(_, id)| *id).collect();
        let cursors = UserHsCursor::read(&user_id_strs, hs_id).await?;

        let users = valid_users
            .into_iter()
            .zip(cursors)
            .map(|((pk, _), cursor)| (pk, EventCursor::new(cursor)))
            .collect();

        Ok(users)
    }

    /// Fetches and processes events for a batch of users in a single
    /// `/events-stream` call, with 404 recovery and batch-wide 429 backoff.
    /// - Fetches with [`Self::fetch_events_with_404_recovery`] (binary-splits
    ///   any missing-user 404s down to the offending leaf user and records only
    ///   those users into the per-user backoff).
    /// - Fan-outs the interleaved events to per-user handlers via
    ///   [`Self::process_batch_events`], persisting each user's cursor.
    /// - Propagates batch-atomic validation errors before per-user bookkeeping;
    ///   these errors are detected before any handler or cursor write runs.
    /// - Records `record_success` for users that completed without fatal error,
    ///   clearing any stale 404 backoff.
    /// - After recording every user's safe progress, propagates the first
    ///   `should_not_retry_now` error so the caller can abort the HS run;
    ///   recoverable per-user errors are logged and skipped.
    #[tracing::instrument(name = "dx.batch_events.process", skip_all, fields(user_count = users.len()))]
    async fn process_users(
        &self,
        hs_pk: &PublicKey,
        users: &[(PublicKey, EventCursor)],
    ) -> Result<(), EventProcessorError> {
        let hs_id: &str = self.homeserver_id.as_ref();
        let user_entries: Vec<(&PublicKey, Option<EventCursor>)> = users
            .iter()
            .map(|(pk, cursor)| (pk, Some(*cursor)))
            .collect();

        let fetch = self
            .fetch_events_with_404_recovery(hs_pk, &user_entries)
            .await?;

        // Convert to the `(&PublicKey, cursor_floor)` format expected by
        // `process_batch_events`, keeping references into `users` alive.
        let batch_users: Vec<(&PublicKey, u64)> =
            users.iter().map(|(pk, cursor)| (pk, cursor.id())).collect();
        let batch_results = self
            .process_batch_events(&batch_users, fetch.events)
            .await?;

        // Users whose fetch 404'd were already recorded into the backoff by
        // `fetch_events_with_404_recovery`; do NOT record success for them, or
        // their fresh skip budget would be wiped. O(1) membership via HashSet
        // instead of a per-user Vec scan.
        let backed_off: HashSet<&PublicKey> = fetch.backed_off_users.iter().collect();

        // `process_batch_events` has already run every user's handlers. Record
        // every safe cursor and success before propagating the first fatal
        // result, otherwise users ordered after it would repeat completed work.
        let mut fatal_error = None;
        for res in batch_results {
            if backed_off.contains(&res.user_pk) {
                continue;
            }

            let user_id = res.user_pk.z32();
            if let Some(cursor_val) = res.latest_cursor {
                if let Err(write_err) = UserHsCursor::write(&user_id, hs_id, cursor_val).await {
                    error!(
                        %user_id, %cursor_val, ?write_err,
                        "Best-effort cursor persist failed; events may be re-processed on next run",
                    );
                }
            }

            match res.result {
                Ok(()) => {
                    self.user_not_found_backoff
                        .record_success(&res.user_pk)
                        .await;
                }
                Err(err) => {
                    if err.should_not_retry_now() {
                        error!(%user_id, ?err, "Fatal error processing user events in batch");
                        if fatal_error.is_none() {
                            fatal_error = Some(err);
                        }
                        continue;
                    }
                    error!(%user_id, ?err, "Recoverable error processing user events in batch; continuing");
                }
            }
        }

        match fatal_error {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    /// Processes already-fetched events for a batch of users from a single
    /// merged, interleaved event stream.
    ///
    /// Ordering note: after a 404 split, the concatenated event list is only
    /// per-sub-batch ordered, not globally ordered by event ID. Per-user cursor
    /// validation is unaffected because each user's events live in exactly one
    /// sub-batch.
    ///
    /// The incoming `stream_events` may be interleaved across users. This function:
    /// 1. Validates that every event's `resource.owner` is in `users`. An event from
    ///    an unexpected user rejects the whole batch with
    ///    [`BatchProcessingError::UnexpectedBatchUser`] before any cursor advances.
    /// 2. Partitions events into per-user subsequences, preserving original relative order.
    /// 3. Validates each user's event subsequence for strict cursor monotonicity
    ///    against their `persisted_cursor` floor. Any user with an out-of-order cursor
    ///    gets an [`EventProcessorError::EventCursorOutOfOrder`] and no cursor advance,
    ///    while other users in the batch continue unaffected.
    /// 4. Executes handlers per user and tracks the highest successfully processed cursor.
    ///    If shutdown is signaled, processing stops cleanly and partial progress is returned.
    pub async fn process_batch_events(
        &self,
        users: &[(&PublicKey, u64)],
        stream_events: Vec<StreamEvent>,
    ) -> Result<Vec<UserBatchResult>, BatchProcessingError> {
        let hs_id: &str = self.homeserver_id.as_ref();

        let mut events_by_user: HashMap<&PublicKey, Vec<StreamEvent>> =
            users.iter().map(|(pk, _)| (*pk, Vec::new())).collect();

        // 1. Membership validation and partitioning
        for event in stream_events {
            let owner = &event.resource.owner;
            if let Some(user_events) = events_by_user.get_mut(owner) {
                user_events.push(event);
            } else {
                return Err(BatchProcessingError::UnexpectedBatchUser {
                    hs_id: self.homeserver_id.to_string(),
                    event_user_id: owner.z32(),
                });
            }
        }

        // 2. Process each user's partitioned subsequence
        let mut results = Vec::with_capacity(users.len());

        for (user_pk, persisted_cursor) in users {
            let user_id = user_pk.z32();
            let events = events_by_user.remove(user_pk).unwrap_or_default();

            let (latest_cursor, result) = self
                .process_user_events(hs_id, &user_id, *persisted_cursor, events)
                .await;

            results.push(UserBatchResult {
                user_pk: (**user_pk).clone(),
                latest_cursor,
                result,
            });
        }

        Ok(results)
    }

    /// Fetches events for a batch of users, retrying 429s with the internal
    /// backoff schedule [`FETCH_EVENTS_429_BACKOFF_SECS`].
    ///
    /// A persistent 429 exhausts the retries into
    /// [`EventProcessorError::HsEventsStreamRateLimitExhausted`], which is
    /// `should_not_retry_now`, so the whole homeserver run aborts and the
    /// runner applies its own per-HS backoff. Any other error propagates
    /// immediately without retry.
    async fn fetch_batch_events_with_429_backoff(
        &self,
        hs_pk: &PublicKey,
        users: &[(&PublicKey, Option<EventCursor>)],
    ) -> Result<Vec<StreamEvent>, EventProcessorError> {
        let mut retry_index = 0;

        loop {
            match self
                .event_source
                .fetch_events(hs_pk, users, self.limit)
                .await
            {
                Ok(events) => return Ok(events),
                Err(err) if err.is_too_many_requests() => {
                    let Some(backoff_secs) = FETCH_EVENTS_429_BACKOFF_SECS.get(retry_index) else {
                        return Err(EventProcessorError::HsEventsStreamRateLimitExhausted);
                    };

                    warn!(
                        batch_size = users.len(),
                        retry_after_secs = *backoff_secs,
                        "Homeserver rate-limited user batch fetch; retrying",
                    );

                    tokio::time::sleep(Duration::from_secs(*backoff_secs)).await;
                    retry_index += 1;
                }
                Err(err) => return Err(err),
            }
        }
    }

    /// Fetches events for a batch of users, isolating users the homeserver
    /// reports as missing (HTTP 404) via recursive-style binary split.
    ///
    /// The homeserver answers a multi-user `/events-stream` request with a
    /// single 404 if **any** requested user is unknown to it, without naming
    /// the offender. To keep the per-user 404 backoff fair without giving up
    /// batching, a 404 on a multi-user batch is split in half and each half
    /// retried independently. Once a 404 surfaces on a single-user leaf, the
    /// user's backoff is updated and it contributes an empty event list so the
    /// healthy parts of the batch still proceed.
    ///
    /// Implemented iteratively with an explicit worklist — equivalent to the
    /// natural recursion but without pulling in an `async_recursion` macro
    /// dependency.
    ///
    /// Returns the fetched events plus the users that were recorded as 404;
    /// callers must not record success for the latter.
    ///
    /// Non-404 errors propagate immediately. 429 exhaustion inside a
    /// sub-batch also propagates, aborting the homeserver run.
    ///
    /// Shutdown is checked between worklist iterations; if signaled, the
    /// already-fetched events and 404-recorded users are returned as partial
    /// results (consistent with `process_batch_events`'s partial-progress-on-
    /// shutdown semantics). Processed user cursors remain valid because the
    /// per-user floor check is independent of other users' fetch order.
    pub async fn fetch_events_with_404_recovery(
        &self,
        hs_pk: &PublicKey,
        users: &[(&PublicKey, Option<EventCursor>)],
    ) -> Result<BatchFetchEvents, EventProcessorError> {
        let mut events = Vec::new();
        let mut backed_off_users = Vec::new();
        if users.is_empty() {
            return Ok(BatchFetchEvents {
                events,
                backed_off_users,
            });
        }

        // Worklist of pending user sub-batches. Items are index ranges into `users`.
        let mut pending: VecDeque<(usize, usize)> = VecDeque::from([(0, users.len())]);

        while let Some((start, end)) = pending.pop_front() {
            if *self.shutdown_rx.borrow() {
                debug!("Shutdown detected; aborting 404-recovery worklist and returning partial results");
                break;
            }

            let batch = &users[start..end];

            match self.fetch_batch_events_with_429_backoff(hs_pk, batch).await {
                Ok(batch_events) => {
                    for event in &batch_events {
                        let owner = &event.resource.owner;
                        if !batch.iter().any(|(user_pk, _)| *user_pk == owner) {
                            return Err(BatchProcessingError::UnexpectedBatchUser {
                                hs_id: self.homeserver_id.to_string(),
                                event_user_id: owner.z32(),
                            }
                            .into());
                        }
                    }
                    events.extend(batch_events);
                }
                Err(err) if err.is_not_found() => {
                    if batch.len() == 1 {
                        let (user_pk, _) = batch[0];
                        self.user_not_found_backoff.record_failure(user_pk).await;
                        warn!(
                            user_id = %user_pk.z32(), action = "skip_user", ?err,
                            "User event fetch returned 404; backing off this user for future runs",
                        );
                        // Missing user contributes no events; continue with the rest.
                        backed_off_users.push((*user_pk).clone());
                    } else {
                        let mid = batch.len() / 2;
                        debug!(
                            total = batch.len(),
                            left = mid,
                            right = batch.len() - mid,
                            "404 on user batch fetch; binary-splitting to isolate missing user(s)",
                        );
                        // Preserve depth-first order so fetches stay roughly in
                        // the original user order, which is gentler on the HS
                        // and keeps logs easier to follow.
                        pending.push_front((start + mid, end));
                        pending.push_front((start, start + mid));
                    }
                }
                Err(err) => return Err(err),
            }
        }

        Ok(BatchFetchEvents {
            events,
            backed_off_users,
        })
    }

    /// Processes already-fetched events for a single user stream.
    ///
    /// `persisted_cursor` is the user's currently stored cursor for this
    /// homeserver; it acts as the initial ordering floor so that events at or
    /// below what we have already indexed are rejected. The complete batch is
    /// validated before any handler runs.
    ///
    /// Returns the latest cursor that is safe to persist, plus the processing
    /// result. Cursor advancement is intentionally skipped for validation
    /// errors and handler errors so those events are fetched again on the next run.
    async fn process_user_events(
        &self,
        hs_id: &str,
        user_id: &str,
        persisted_cursor: u64,
        stream_events: Vec<StreamEvent>,
    ) -> (Option<u64>, Result<(), EventProcessorError>) {
        if *self.shutdown_rx.borrow() {
            debug!(%user_id, "Shutdown detected; exiting event loop");
            return (None, Ok(()));
        }

        let mut cursor_floor = persisted_cursor;
        for stream_event in &stream_events {
            let cursor_id = stream_event.cursor.id();
            if cursor_id <= cursor_floor {
                OUT_OF_ORDER_CURSOR_EXTERNAL_HS
                    .add(1, &[KeyValue::new("hs_id", hs_id.to_string())]);
                return (
                    None,
                    Err(EventProcessorError::EventCursorOutOfOrder {
                        hs_id: hs_id.into(),
                        user_id: user_id.into(),
                        cursor: cursor_id,
                        cursor_floor,
                    }),
                );
            }
            cursor_floor = cursor_id;
        }

        let mut latest_cursor: Option<u64> = None;

        for stream_event in stream_events {
            if *self.shutdown_rx.borrow() {
                debug!(%user_id, "Shutdown detected; exiting event loop");
                break;
            }

            let cursor_id = stream_event.cursor.id();

            // External homeservers must not index another user's URI.
            // Defense-in-depth: after batch partitioning, events reach
            // this check only via process_batch_events' owner-keyed buckets,
            // so `owner == user_id` holds by construction — a foreign owner is
            // already rejected as `BatchProcessingError::UnexpectedBatchUser`.
            // The check stays deliberately (from the #997 hardening) as the last-resort guard
            // against a foreign-user event silently advancing the cursor via
            // the unsupported-path skip, before [Event::from_stream_event].
            if let Err(err) = Self::validate_user_id(hs_id, &stream_event, user_id) {
                return (latest_cursor, Err(err));
            }

            match Event::from_stream_event(&stream_event) {
                Ok(Some(event)) => {
                    if let Err(err) = self.handle_event(&event).await {
                        return (latest_cursor, Err(err));
                    }
                }
                Ok(None) => { /* resource not handled by Nexus, skip */ }
                Err(e) => {
                    error!(
                        %user_id,
                        %cursor_id,
                        error = %e,
                        "Skipping unparseable stream event"
                    );
                }
            }

            // Advance after successful handling, unsupported resources, or
            // logged parse errors. Validation errors and handler errors return
            // before this point, so their cursor is not persisted.
            latest_cursor = Some(cursor_id);
        }

        (latest_cursor, Ok(()))
    }

    fn validate_user_id(
        hs_id: &str,
        stream_event: &StreamEvent,
        expected_user_id: &str,
    ) -> Result<(), EventProcessorError> {
        let event_user_id = stream_event.resource.owner.z32();
        if event_user_id != expected_user_id {
            return Err(EventProcessorError::UserIdMismatch {
                hs_id: hs_id.into(),
                expected_user_id: expected_user_id.into(),
                event_user_id,
            });
        }

        Ok(())
    }
}
