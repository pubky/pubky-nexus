use std::{path::PathBuf, sync::Arc, time::Duration};

use crate::errors::EventProcessorError;
use crate::events::Event;
use futures::StreamExt;
use nexus_common::db::PubkyConnector;
use nexus_common::models::homeserver::{Homeserver, HsBlacklist};
use nexus_common::models::user::UserHsCursor;
use pubky::{Event as StreamEvent, EventCursor, PublicKey};
use tokio::sync::watch::Receiver;
use tracing::{debug, error, info, warn};

use super::TEventProcessor;
use crate::events::retry::RetryScheduler;
use crate::events::EventHandler;
use crate::service::runner::UserNotFoundBackoff;
use crate::service::user_hs_resolver;

const FETCH_EVENTS_429_BACKOFF_SECS: [u64; 3] = [1, 2, 3];

#[async_trait::async_trait]
pub trait KeyBasedEventSource: Send + Sync + 'static {
    async fn fetch_events(
        &self,
        hs_pk: &PublicKey,
        user_pk: &PublicKey,
        cursor: EventCursor,
        limit: u16,
    ) -> Result<Vec<StreamEvent>, EventProcessorError>;
}

pub struct PubkyKeyBasedEventSource;

#[async_trait::async_trait]
impl KeyBasedEventSource for PubkyKeyBasedEventSource {
    async fn fetch_events(
        &self,
        hs_pk: &PublicKey,
        user_pk: &PublicKey,
        cursor: EventCursor,
        limit: u16,
    ) -> Result<Vec<StreamEvent>, EventProcessorError> {
        let pubky = PubkyConnector::get()?;

        // We are building the stream without the live flag, so it performs an HTTP GET and closes.
        // See rustdoc of EventStreamBuilder::live()
        let mut stream = pubky
            .event_stream_for(hs_pk)
            .add_users(vec![(user_pk, Some(cursor))])?
            .limit(limit)
            .path("/pub/")
            .subscribe()
            .await
            .inspect_err(|e| error!("Failed to subscribe to event stream: {e:?}"))?;

        let mut events = Vec::new();
        while let Some(result) = stream.next().await {
            events.push(result?);
        }

        Ok(events)
    }
}

/// Event processor for non-default HSs, where the user-specific `/events-stream` endpoint is used
pub struct KeyBasedEventProcessor {
    /// The HS endpoint this processor fetches events from
    pub homeserver: Homeserver,

    /// Max events the homeserver will send before closing the stream.
    /// Bounds execution time per user, preventing timeout and starvation.
    pub limit: u16,

    pub files_path: PathBuf,
    pub event_handler: Arc<dyn EventHandler>,
    pub event_source: Arc<dyn KeyBasedEventSource>,
    pub user_not_found_backoff: Arc<UserNotFoundBackoff>,
    pub max_file_size: u64,

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
    fn files_path(&self) -> &PathBuf {
        &self.files_path
    }

    fn event_handler(&self) -> &Arc<dyn EventHandler> {
        &self.event_handler
    }

    fn instance_name(&self) -> String {
        format!("KeyBasedEventProcessor with HS ID: {}", self.homeserver.id)
    }

    fn retry_scheduler(&self) -> Option<&Arc<RetryScheduler>> {
        Some(&self.retry_scheduler)
    }

    fn homeserver_id(&self) -> Option<&str> {
        Some(self.homeserver.id.as_ref())
    }

    fn max_file_size(&self) -> u64 {
        self.max_file_size
    }

    async fn run_internal(self: Arc<Self>) -> Result<(), EventProcessorError> {
        let hs_id = self.homeserver.id.to_string();

        // Blacklisted HSs must never be indexed. The runner already excludes
        // them from `pre_run`, so reaching here is unexpected.
        if self.hs_blacklist.is_blacklisted(&hs_id) {
            error!(%hs_id, action = "abort_hs", "Refusing to process blacklisted HS");
            return Err(EventProcessorError::HsBlacklisted { hs_id });
        }

        let hs_pk = self.homeserver.id.to_public_key();

        let users = self
            .resolve_users_with_cursors(&hs_id)
            .await
            .inspect_err(|e| error!("Failed to resolve users: {e:?}"))?;

        if users.is_empty() {
            debug!("No users, skipping");
            return Ok(());
        }

        info!("Found {} users", users.len());

        for (user_pk, cursor) in &users {
            if *self.shutdown_rx.borrow() {
                debug!("Shutdown detected; stopping user iteration");
                break;
            }
            let user_id = user_pk.z32();

            // Users whose event fetch previously returned 404 are skipped for an
            // increasing number of runs (see `UserNotFoundBackoff`).
            if self.user_not_found_backoff.consume_skip(user_pk).await {
                debug!(
                    %hs_id, %user_id, action = "skip_user",
                    "Skipping user due to prior 404 (NotFound404) backoff",
                );
                continue;
            }

            match self.process_user(&hs_pk, &hs_id, user_pk, *cursor).await {
                Ok(()) => self.user_not_found_backoff.record_success(user_pk).await,
                Err(err) => {
                    if err.should_not_retry_now() {
                        error!(
                            %hs_id, %user_id, action = "abort_hs", ?err,
                            "Got should-not-retry-now error while processing user; aborting homeserver run",
                        );
                        return Err(err);
                    }

                    if err.is_not_found() {
                        self.user_not_found_backoff.record_failure(user_pk).await;
                        warn!(
                            %hs_id, %user_id, action = "skip_user", ?err,
                            "User event fetch returned 404; backing off this user for future runs",
                        );
                    } else {
                        error!(
                            %hs_id, %user_id, action = "skip_user", ?err,
                            "Got error while processing user; continuing with next user",
                        );
                    }
                }
            }
        }

        Ok(())
    }
}

impl KeyBasedEventProcessor {
    /// Resolves monitored users on this homeserver and reads their cursors from Redis.
    #[tracing::instrument(name = "dx.users.resolve", skip_all, fields(homeserver = %hs_id))]
    async fn resolve_users_with_cursors(
        &self,
        hs_id: &str,
    ) -> Result<Vec<(PublicKey, EventCursor)>, EventProcessorError> {
        let user_ids = user_hs_resolver::get_user_ids_by_homeserver(hs_id).await?;
        debug!("Resolved {} user(s)", user_ids.len());

        let mut valid_users: Vec<(PublicKey, &str)> = Vec::with_capacity(user_ids.len());
        for user_id in &user_ids {
            let Ok(user_pk) = user_id.parse::<PublicKey>() else {
                warn!("Invalid user public key '{user_id}', skipping");
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
    #[tracing::instrument(name = "dx.user_events.process", skip_all, fields(
        homeserver = %hs_id,
        user = %user_pk.z32(),
    ))]
    async fn process_user(
        &self,
        hs_pk: &PublicKey,
        hs_id: &str,
        user_pk: &PublicKey,
        cursor: EventCursor,
    ) -> Result<(), EventProcessorError> {
        let stream_events = self
            .fetch_user_events_with_429_backoff(hs_pk, hs_id, user_pk, cursor)
            .await?;

        let user_id = user_pk.z32();
        let (latest_cursor, result) = self
            .process_user_events(hs_id, &user_id, stream_events)
            .await;

        if let Some(cursor_val) = latest_cursor {
            if let Err(write_err) = UserHsCursor::write(&user_id, hs_id, cursor_val).await {
                error!(
                    %hs_id, %user_id, %cursor_val, ?write_err,
                    "Best-effort cursor persist failed; events may be re-processed on next run",
                );
            }
        }

        result
    }

    async fn fetch_user_events_with_429_backoff(
        &self,
        hs_pk: &PublicKey,
        hs_id: &str,
        user_pk: &PublicKey,
        cursor: EventCursor,
    ) -> Result<Vec<StreamEvent>, EventProcessorError> {
        let user_id = user_pk.z32();
        let mut retry_index = 0;

        loop {
            match self
                .event_source
                .fetch_events(hs_pk, user_pk, cursor, self.limit)
                .await
            {
                Ok(events) => return Ok(events),
                Err(err) if err.is_too_many_requests() => {
                    let Some(backoff_secs) = FETCH_EVENTS_429_BACKOFF_SECS.get(retry_index) else {
                        return Err(EventProcessorError::HsEventsStreamRateLimitExhausted);
                    };

                    warn!(
                        %hs_id, %user_id, retry_after_secs = *backoff_secs,
                        "Homeserver rate-limited user event fetch; retrying",
                    );

                    tokio::time::sleep(Duration::from_secs(*backoff_secs)).await;
                    retry_index += 1;
                }
                Err(err) => return Err(err),
            }
        }
    }

    /// Processes already-fetched events for a single user stream.
    ///
    /// Returns the latest cursor that is safe to persist, plus the processing
    /// result. Cursor advancement is intentionally skipped for `UserIdMismatch`
    /// and handler errors so those events are fetched again on the next run.
    async fn process_user_events(
        &self,
        hs_id: &str,
        user_id: &str,
        stream_events: Vec<StreamEvent>,
    ) -> (Option<u64>, Result<(), EventProcessorError>) {
        let mut latest_cursor: Option<u64> = None;

        for stream_event in stream_events {
            if *self.shutdown_rx.borrow() {
                debug!(hs_id = %hs_id, user = %user_id, "Shutdown detected; exiting event loop");
                break;
            }

            let cursor_id = stream_event.cursor.id();

            match Event::from_stream_event(&stream_event, self.files_path.clone()) {
                Ok(Some(event)) => {
                    // External homeservers must not index another user's URI.
                    if let Err(err) = Self::validate_user_id(hs_id, &event, user_id) {
                        return (latest_cursor, Err(err));
                    }

                    if let Err(err) = self.handle_event(&event).await {
                        return (latest_cursor, Err(err));
                    }
                }
                Ok(None) => { /* resource not handled by Nexus, skip */ }
                Err(e) => {
                    error!(%hs_id, %user_id, %cursor_id, "Skipping unparseable stream event: {e}");
                }
            }

            // Advance after successful handling, unsupported resources, or
            // logged parse errors. UserIdMismatch and handler errors return
            // before this point, so their cursor is not persisted.
            latest_cursor = Some(cursor_id);
        }

        (latest_cursor, Ok(()))
    }

    fn validate_user_id(
        hs_id: &str,
        event: &Event,
        expected_user_id: &str,
    ) -> Result<(), EventProcessorError> {
        let event_user_id = event.parsed_uri.user_id().to_string();
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
