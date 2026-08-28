use std::collections::{HashMap, VecDeque};

use tokio::sync::Mutex;

use nexus_watcher::errors::EventProcessorError;
use nexus_watcher::service::indexer::KeyBasedEventSource;
use pubky::{Event as StreamEvent, EventCursor, PublicKey};

type FetchEventsResult = Result<Vec<StreamEvent>, EventProcessorError>;

/// A recorded call to `fetch_events`, capturing the requested user batch and limit.
#[derive(Debug, Clone)]
pub struct FetchCall {
    /// User IDs and their cursors in the requested batch (`None` = from beginning).
    pub users: Vec<(String, Option<u64>)>,
    /// The `limit` parameter passed to the fetch.
    pub limit: u16,
}

#[derive(Default)]
pub struct MockKeyBasedEventSource {
    /// Event batches returned in fetch order.
    /// Useful when user ordering is not important and tests only care about processor flow.
    events: Mutex<VecDeque<FetchEventsResult>>,

    /// Event batches returned by requested user ID.
    /// Useful when graph user ordering is intentionally not part of the assertion.
    /// Every keyed user in a fetched batch must have an entry for the fixture
    /// to apply; events are merged in requested-user order.
    user_events: Mutex<HashMap<String, FetchEventsResult>>,

    /// Batches requested from the mock, in fetch order.
    /// Useful for asserting the processor continued to, or stopped before, specific users.
    recorded_batches: Mutex<Vec<FetchCall>>,
}

impl MockKeyBasedEventSource {
    pub fn with_events(mut self, events: Vec<Vec<StreamEvent>>) -> Self {
        *self.events.get_mut() = events.into_iter().map(Ok).collect();
        self
    }

    pub fn with_results(mut self, results: Vec<FetchEventsResult>) -> Self {
        *self.events.get_mut() = results.into();
        self
    }

    pub fn with_user_events(mut self, events: Vec<(String, Vec<StreamEvent>)>) -> Self {
        *self.user_events.get_mut() = events
            .into_iter()
            .map(|(user_id, events)| (user_id, Ok(events)))
            .collect();
        self
    }

    pub fn with_user_results(mut self, results: Vec<(String, FetchEventsResult)>) -> Self {
        *self.user_events.get_mut() = results.into_iter().collect();
        self
    }

    /// Returns the raw batch calls, preserving batch structure.
    pub async fn batch_calls(&self) -> Vec<FetchCall> {
        self.recorded_batches.lock().await.clone()
    }
}

#[async_trait::async_trait]
impl KeyBasedEventSource for MockKeyBasedEventSource {
    async fn fetch_events(
        &self,
        _hs_pk: &PublicKey,
        users: &[(&PublicKey, Option<EventCursor>)],
        limit: u16,
    ) -> Result<Vec<StreamEvent>, EventProcessorError> {
        let batch: Vec<(String, Option<u64>)> = users
            .iter()
            .map(|(pk, cursor)| (pk.z32(), cursor.map(|c| c.id())))
            .collect();

        self.recorded_batches.lock().await.push(FetchCall {
            users: batch.clone(),
            limit,
        });

        // User-keyed fixtures apply when every requested user has an entry.
        // An error result fails that whole batched fetch *repeatedly* — it is
        // sticky (not consumed), mirroring a real HS where a missing (404) or
        // temporarily failing user keeps erroring every sub-batch that still
        // contains it, until the caller stops requesting the user (e.g. via
        // the 404 backoff). On success, events are merged in requested-user
        // order and their entries consumed.
        if !batch.is_empty() {
            let mut keyed = self.user_events.lock().await;
            if batch.iter().all(|(user_id, _)| keyed.contains_key(user_id)) {
                if let Some(err) = batch
                    .iter()
                    .filter_map(|(user_id, _)| keyed[user_id].as_ref().err())
                    .next()
                {
                    return Err(err.clone());
                }

                let mut events = Vec::new();
                for (user_id, _) in &batch {
                    let user_events = keyed
                        .remove(user_id)
                        .expect("checked contains_key")
                        .expect("no errors left");
                    events.extend(user_events);
                }
                return Ok(events);
            }
        }

        // Once the queued results are exhausted, further fetches resolve to an
        // empty success. Tests that model a persistently failing user must queue
        // one result per expected fetch (see the 404 backoff tests).
        self.events
            .lock()
            .await
            .pop_front()
            .unwrap_or_else(|| Ok(Vec::new()))
    }
}
