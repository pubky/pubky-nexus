use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use futures::StreamExt;
use nexus_common::db::{PubkyConnector, RedisOps};
use nexus_common::models::event::{Event, EventProcessorError};
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::user::{UserDetails, USER_HS_CURSOR};
use pubky::{EventCursor, PublicKey};
use tokio::sync::watch::Receiver;
use tracing::{debug, error, info};

use super::TEventProcessor;
use crate::events::Moderation;
use crate::service::user_hs_resolver;

/// Max users per `EventStreamBuilder` subscription (SDK-enforced limit).
const MAX_USERS_PER_STREAM: usize = 50;

/// Event processor for non-default HSs, where the user-specific `/events-stream` endpoint is used
pub struct KeyBasedEventProcessor {
    /// The HS endpoint this processor fetches events from
    /// TODO Used in X1 (see mod.rs)
    pub homeserver: Homeserver,

    /// Max events the homeserver will send before closing the stream.
    /// Bounds execution time per batch, preventing timeout and starvation.
    pub limit: u16,
    pub files_path: PathBuf,
    pub moderation: Arc<Moderation>,
    pub shutdown_rx: Receiver<bool>,
}

#[async_trait::async_trait]
impl TEventProcessor for KeyBasedEventProcessor {
    fn files_path(&self) -> &PathBuf {
        &self.files_path
    }

    fn moderation(&self) -> &Arc<Moderation> {
        &self.moderation
    }

    async fn run_internal(self: Arc<Self>) -> Result<(), EventProcessorError> {
        let hs_id = self.homeserver.id.to_string();

        let hs_pk: PublicKey = hs_id.parse().map_err(|_| {
            EventProcessorError::client_error(format!(
                "Invalid homeserver public key: {hs_id}"
            ))
        })?;

        let users = self
            .resolve_users_with_cursors(&hs_id)
            .await
            .inspect_err(|e| error!("Failed to resolve users for HS {hs_id}: {e:?}"))?;

        if users.is_empty() {
            debug!("No users on HS {hs_id}, skipping");
            return Ok(());
        }

        info!("Found {} users on HS {hs_id}", users.len());

        for batch in users.chunks(MAX_USERS_PER_STREAM) {
            self.process_batch(&hs_pk, &hs_id, batch)
                .await
                .inspect_err(|e| error!("Batch failed for HS {hs_id}: {e:?}"))?;
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
        debug!("Resolved {} user(s) on HS {hs_id}", user_ids.len());

        let mut users = Vec::with_capacity(user_ids.len());
        for user_id in &user_ids {
            let user_pk: PublicKey = user_id.parse().map_err(|_| {
                EventProcessorError::client_error(format!("Invalid user public key: {user_id}"))
            })?;
            let cursor = Self::read_user_cursor(user_id, hs_id).await?;
            users.push((user_pk, cursor));
        }

        Ok(users)
    }

    /// Subscribes to the event stream for a batch of users and processes incoming events.
    ///
    /// Cursors are accumulated in memory and flushed to Redis once at the end.
    /// On error, partial cursor progress is persisted before re-raising.
    #[tracing::instrument(name = "dx.event_batch.process", skip_all, fields(
        homeserver = %hs_id,
        batch.size = batch.len(),
    ))]
    async fn process_batch(
        &self,
        hs_pk: &PublicKey,
        hs_id: &str,
        batch: &[(PublicKey, EventCursor)],
    ) -> Result<(), EventProcessorError> {
        let user_refs: Vec<(&PublicKey, Option<EventCursor>)> = batch
            .iter()
            .map(|(pk, cursor)| (pk, Some(*cursor)))
            .collect();

        let pubky = PubkyConnector::get()?;
        let mut stream = pubky
            .event_stream_for(hs_pk)
            .add_users(user_refs)?
            .limit(self.limit)
            .path("/pub/")
            .subscribe()
            .await
            .inspect_err(|e| error!("Failed to subscribe to event stream: {e:?}"))?;

        let mut cursors: HashMap<String, u64> = HashMap::new();

        let batch_result: Result<(), EventProcessorError> = async {
            while let Some(result) = stream.next().await {
                if *self.shutdown_rx.borrow() {
                    debug!(hs_id = %hs_id, "Shutdown detected; exiting event processing loop");
                    break;
                }

                let stream_event = result?;

                if let Some(event) = Event::from_stream_event(&stream_event, self.files_path.clone())? {
                    self.handle_event(&event).await?;
                }

                cursors.insert(stream_event.resource.owner.z32(), stream_event.cursor.id());
            }
            Ok(())
        }
        .await;

        Self::flush_cursors(&cursors, hs_id).await?;
        batch_result
    }

    /// Reads the per-user event cursor from the `USER_HS_CURSOR` sorted set in Redis.
    ///
    /// Returns `EventCursor(0)` when the user has no cursor entry (newly ingested).
    /// Propagates Redis errors instead of silently rewinding to 0.
    ///
    /// The cursor is stored as the score (f64) of the homeserver member.
    /// f64 is exact for integer values up to 2^53 (~9 quadrillion), which is
    /// practically unreachable for monotonically incrementing event IDs.
    async fn read_user_cursor(
        user_id: &str,
        hs_id: &str,
    ) -> Result<EventCursor, EventProcessorError> {
        let cursor_key = [&USER_HS_CURSOR[..], &[user_id]].concat();
        let score = UserDetails::check_sorted_set_member(None, &cursor_key, &[hs_id]).await?;
        Ok(EventCursor::new(score.unwrap_or(0) as u64))
    }

    /// Persists the per-user event cursor back to the `USER_HS_CURSOR` sorted set.
    async fn write_user_cursor(
        user_id: &str,
        hs_id: &str,
        cursor: u64,
    ) -> Result<(), EventProcessorError> {
        let cursor_key = [&USER_HS_CURSOR[..], &[user_id]].concat();
        UserDetails::put_index_sorted_set(
            &cursor_key,
            &[(cursor as f64, hs_id)],
            None,
            None,
        )
        .await?;
        Ok(())
    }

    /// Flushes all accumulated in-memory cursors to Redis in one pass.
    async fn flush_cursors(
        cursors: &HashMap<String, u64>,
        hs_id: &str,
    ) -> Result<(), EventProcessorError> {
        for (user_id, cursor) in cursors {
            Self::write_user_cursor(user_id, hs_id, *cursor).await?;
        }
        Ok(())
    }
}
