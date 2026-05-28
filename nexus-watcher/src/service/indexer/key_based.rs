use std::{path::PathBuf, sync::Arc, time::Duration};

use futures::StreamExt;
use nexus_common::db::{PubkyConnector, RedisOps};
use nexus_common::models::event::{Event, EventProcessorError};
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::user::{user_hs_cursor_key, UserDetails};
use pubky::{Event as StreamEvent, EventCursor, PublicKey};
use tokio::sync::watch::Receiver;
use tracing::{debug, error, info, warn};

use super::TEventProcessor;
use crate::events::retry::RetryScheduler;
use crate::events::EventHandler;
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

    async fn run_internal(self: Arc<Self>) -> Result<(), EventProcessorError> {
        let hs_id = self.homeserver.id.to_string();

        let hs_pk: PublicKey = hs_id.parse().map_err(|_| {
            EventProcessorError::client_error("Invalid homeserver public key".to_string())
        })?;

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

            if let Err(err) = self.process_user(&hs_pk, &hs_id, user_pk, *cursor).await {
                let user_id = user_pk.z32();
                if err.is_infrastructure() {
                    error!(
                        hs_id = %hs_id,
                        user = %user_id,
                        action = "abort_hs",
                        error = ?err,
                        "Infrastructure error while processing user; aborting homeserver run",
                    );
                    return Err(err);
                }

                error!(
                    hs_id = %hs_id,
                    user = %user_id,
                    action = "skip_user",
                    error = ?err,
                    "Non-infrastructure user error; continuing with next user",
                );
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
        let cursors = Self::read_users_cursors(&user_id_strs, hs_id).await?;

        let users = valid_users
            .into_iter()
            .zip(cursors)
            .map(|((pk, _), cursor)| (pk, cursor))
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
            if let Err(write_err) = Self::write_user_cursor(&user_id, hs_id, cursor_val).await {
                error!(
                    hs_id = %hs_id,
                    user = %user_id,
                    cursor = cursor_val,
                    cursor_write_error = ?write_err,
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
                        return Err(err);
                    };

                    warn!(
                        hs_id = %hs_id,
                        user = %user_id,
                        retry_after_secs = *backoff_secs,
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
        let event_user_id = event.parsed_uri.user_id().as_str();
        if event_user_id != expected_user_id {
            return Err(EventProcessorError::UserIdMismatch {
                hs_id: hs_id.into(),
                expected_user_id: expected_user_id.into(),
                event_user_id: event_user_id.into(),
            });
        }

        Ok(())
    }

    /// Reads per-user event cursors from the `USER_HS_CURSOR` sorted sets in Redis.
    ///
    /// Each user's cursor lives in its own sorted set (keyed by user ID) with
    /// the homeserver ID as the member. All lookups are batched into a single
    /// `check_sorted_set_members` pipeline call.
    ///
    /// Returns `EventCursor(0)` for users with no cursor entry (newly ingested).
    /// Propagates Redis errors instead of silently rewinding to 0.
    ///
    /// The cursor is stored as the score (f64) of the homeserver member.
    /// f64 is exact for integer values up to 2^53 (~9 quadrillion), which is
    /// practically unreachable for monotonically incrementing event IDs.
    async fn read_users_cursors(
        user_ids: &[&str],
        hs_id: &str,
    ) -> Result<Vec<EventCursor>, EventProcessorError> {
        let keys: Vec<[&str; 3]> = user_ids
            .iter()
            .map(|user_id| user_hs_cursor_key(user_id))
            .collect();
        let member: [&str; 1] = [hs_id];
        let pairs: Vec<(&[&str], &[&str])> = keys
            .iter()
            .map(|k| (k.as_slice(), member.as_slice()))
            .collect();

        let scores = UserDetails::check_sorted_set_members(None, &pairs).await?;

        Ok(scores
            .into_iter()
            .map(|s| EventCursor::new(s.unwrap_or(0) as u64))
            .collect())
    }

    /// Persists the per-user event cursor back to the `USER_HS_CURSOR` sorted set.
    async fn write_user_cursor(
        user_id: &str,
        hs_id: &str,
        cursor: u64,
    ) -> Result<(), EventProcessorError> {
        let key = user_hs_cursor_key(user_id);
        UserDetails::put_index_sorted_set(&key, &[(cursor as f64, hs_id)], None, None).await?;
        Ok(())
    }
}
