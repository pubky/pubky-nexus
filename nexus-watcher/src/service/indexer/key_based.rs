use std::{
    sync::{Arc, LazyLock},
    time::Duration,
};

use tokio::time::Instant;

use crate::errors::EventProcessorError;
use crate::events::Event;
use crate::homeserver::{HomeserverClient, HsStreamEvent};
use nexus_common::models::homeserver::HsBlacklist;
use nexus_common::models::user::UserHsCursor;
use opentelemetry::metrics::Counter;
use opentelemetry::{global, KeyValue};
use pubky::{EventCursor, PublicKey};
use pubky_social_specs::PubkyId;
use tokio::sync::watch::Receiver;
use tracing::{debug, error, info, warn};

use super::{RunOutcome, TEventProcessor};
use crate::events::retry::RetryScheduler;
use crate::events::EventHandler;
use crate::service::runner::{UserNotFoundBackoff, UserResumePoints};
use crate::service::user_hs_resolver;
use crate::service::KEY_BASED_SLOW_RUN_MARGIN;

/// Counter for per-user stream events an External HS returned at or below the
/// ordering floor — a replay of already-indexed data, or a regression against an
/// earlier event in the same batch. The whole batch is rejected before any handler
/// runs. Labelled by `hs_id` only to avoid per-user metric cardinality.
static OUT_OF_ORDER_CURSOR_EXTERNAL_HS: LazyLock<Counter<u64>> = LazyLock::new(|| {
    global::meter(crate::METER_NAME)
        .u64_counter("watcher.external_hs.cursor.out_of_order")
        .with_description("Per-user stream events a homeserver returned out of cursor order")
        .build()
});

/// Event processor for third-party (external) HSs, where the user-specific `/events-stream`
/// endpoint is used
pub struct KeyBasedEventProcessor {
    /// The HS endpoint this processor fetches events from
    pub homeserver_id: PubkyId,

    /// Max events the homeserver will send before closing the stream.
    /// Bounds execution time per user, preventing timeout and starvation.
    pub limit: u16,

    pub event_handler: Arc<dyn EventHandler>,
    pub client: Arc<dyn HomeserverClient>,
    pub user_not_found_backoff: Arc<UserNotFoundBackoff>,

    /// HS PKs that should not be indexed. Defense-in-depth: the runner already
    /// excludes these from `pre_run`, but the processor refuses to run for a
    /// blacklisted HS too.
    pub hs_blacklist: HsBlacklist,

    /// Scheduler used to enqueue failed events onto the retry queue
    pub retry_scheduler: Arc<RetryScheduler>,

    pub shutdown_rx: Receiver<bool>,

    /// Where this homeserver's runs start, so a run that stops at its budget leaves the next
    /// run the users it did not reach
    pub user_resume_points: Arc<UserResumePoints>,

    /// How long a run may spend on this homeserver, so it cannot hold up the other
    /// homeservers of the external loop. Once spent, the run stops after the event in
    /// progress, keeps the cursors it reached, starts no other user, and ends with
    /// [`RunOutcome::BudgetSpent`].
    pub budget: Duration,
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

    async fn run_internal(self: Arc<Self>) -> Result<RunOutcome, EventProcessorError> {
        let hs_id = self.homeserver_id.to_string();

        // Blacklisted HSs must never be indexed. The runner already excludes
        // them from `pre_run`, so reaching here is unexpected.
        if self.hs_blacklist.is_blacklisted(&hs_id) {
            error!(action = "abort_hs", "Refusing to process blacklisted HS");
            return Err(EventProcessorError::HsBlacklisted { hs_id });
        }

        // A format-valid id need not be a curve point, and then no user's stream on it can
        // be read. That never changes, so the run stops here instead of failing every user.
        self.homeserver_id.to_public_key().map_err(|e| {
            EventProcessorError::SpecValidation(format!("{hs_id} is not a valid public key: {e}"))
        })?;
        let deadline = Instant::now() + self.budget;

        let mut users = self
            .resolve_users_with_cursors()
            .await
            .inspect_err(|e| error!(error = ?e, "Failed to resolve users"))?;

        if users.is_empty() {
            debug!("No users, skipping");
            return Ok(RunOutcome::Completed);
        }

        info!(user_count = users.len(), "Found users");
        self.user_resume_points.order(&hs_id, &mut users).await;

        for (index, (user_pk, cursor)) in users.iter().enumerate() {
            if *self.shutdown_rx.borrow() {
                debug!("Shutdown detected; stopping user iteration");
                return Ok(RunOutcome::Completed);
            }
            let user_id = user_pk.z32();

            // Users whose event fetch previously returned 404 are skipped for an
            // increasing number of runs (see `UserNotFoundBackoff`).
            if self.user_not_found_backoff.consume_skip(user_pk).await {
                debug!(
                    %user_id, action = "skip_user",
                    "Skipping user due to prior 404 (NotFound404) backoff",
                );
                continue;
            }

            match self.process_user(user_pk, *cursor, deadline).await {
                Ok(()) => self.user_not_found_backoff.record_success(user_pk).await,
                Err(err) => {
                    if err.should_not_retry_now() {
                        error!(
                            %user_id, action = "abort_hs", ?err,
                            "Got should-not-retry-now error while processing user; aborting homeserver run",
                        );
                        return Err(err);
                    }

                    if err.is_not_found() {
                        self.user_not_found_backoff.record_failure(user_pk).await;
                        warn!(
                            %user_id, action = "skip_user", ?err,
                            "User event fetch returned 404; backing off this user for future runs",
                        );
                    } else {
                        error!(
                            %user_id, action = "skip_user", ?err,
                            "Got error while processing user; continuing with next user",
                        );
                    }
                }
            }

            // Checked after a user, not before, so a run makes progress even when finding its
            // users spent the budget. A batch the budget cut short also ends here.
            if Instant::now() >= deadline {
                info!(
                    action = "stop_hs",
                    budget_secs = self.budget.as_secs(),
                    users_left = users.len() - index - 1,
                    "Run budget spent; the next run starts from the next user",
                );
                self.user_resume_points
                    .record_stop_after(&hs_id, &users, index)
                    .await;
                return Ok(RunOutcome::BudgetSpent);
            }
        }

        self.user_resume_points.record_full_pass(&hs_id).await;
        Ok(RunOutcome::Completed)
    }

    fn slow_run_threshold(&self) -> Option<Duration> {
        Some(self.budget + KEY_BASED_SLOW_RUN_MARGIN)
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

    /// Subscribes to the event stream for a single user and processes incoming events.
    ///
    /// Each user gets their own `limit` budget, ensuring fair progress regardless
    /// of how many events other users have produced.
    #[tracing::instrument(name = "dx.user_events.process", skip_all, fields(user_id = %user_pk.z32()))]
    async fn process_user(
        &self,
        user_pk: &PublicKey,
        cursor: EventCursor,
        deadline: Instant,
    ) -> Result<(), EventProcessorError> {
        let hs_id: &str = self.homeserver_id.as_ref();
        let stream_events = self
            .client
            .user_events(&self.homeserver_id, user_pk, cursor.id(), self.limit)
            .await?;

        let user_id = user_pk.z32();
        let (latest_cursor, result) = self
            .process_user_events(hs_id, &user_id, cursor.id(), stream_events, deadline)
            .await;

        if let Some(cursor_val) = latest_cursor {
            if let Err(write_err) = UserHsCursor::write(&user_id, hs_id, cursor_val).await {
                error!(
                    %user_id, %cursor_val, ?write_err,
                    "Best-effort cursor persist failed; events may be re-processed on next run",
                );
            }
        }

        result
    }

    /// Processes already-fetched events for a single user stream.
    ///
    /// `persisted_cursor` is the user's currently stored cursor for this
    /// homeserver; it acts as the initial ordering floor so that events at or
    /// below what we have already indexed are rejected. The complete batch is
    /// validated before any handler runs.
    ///
    /// Returns the latest cursor that is safe to persist, plus the processing
    /// result. Cursor advancement is intentionally skipped for `UserIdMismatch`
    /// and handler errors so those events are fetched again on the next run.
    async fn process_user_events(
        &self,
        hs_id: &str,
        user_id: &str,
        persisted_cursor: u64,
        stream_events: Vec<HsStreamEvent>,
        deadline: Instant,
    ) -> (Option<u64>, Result<(), EventProcessorError>) {
        if *self.shutdown_rx.borrow() {
            debug!(%user_id, "Shutdown detected; exiting event loop");
            return (None, Ok(()));
        }

        let mut cursor_floor = persisted_cursor;
        for stream_event in &stream_events {
            let cursor_id = stream_event.cursor;
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

            let cursor_id = stream_event.cursor;

            // External homeservers must not index another user's URI.
            // Validate the raw resource before [Event::from_stream_event],
            // because a foreign user PK with an unsupported path would
            // return Ok(None) thus skipping but also advancing latest_cursor.
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
            // logged parse errors. UserIdMismatch and handler errors return
            // before this point, so their cursor is not persisted.
            latest_cursor = Some(cursor_id);

            // Checked after an event, not before, so every fetched batch makes progress even
            // when the fetch alone spent the budget.
            if Instant::now() >= deadline {
                debug!(%user_id, "Run budget spent; leaving the rest of the batch to the next run");
                break;
            }
        }

        (latest_cursor, Ok(()))
    }

    fn validate_user_id(
        hs_id: &str,
        stream_event: &HsStreamEvent,
        expected_user_id: &str,
    ) -> Result<(), EventProcessorError> {
        let event_user_id = stream_event.owner().z32();
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
