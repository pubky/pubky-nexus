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
    user_events: Mutex<HashMap<String, FetchEventsResult>>,

    /// Batches requested from the mock, in fetch order.
    /// Useful for asserting the processor continued to, or stopped before, specific users.
    calls: Mutex<Vec<FetchCall>>,
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

    /// Returns the user IDs from all recorded calls, flattened across batches.
    pub async fn calls(&self) -> Vec<String> {
        self.calls
            .lock()
            .await
            .iter()
            .flat_map(|call| call.users.iter().map(|(user_id, _)| user_id.clone()))
            .collect()
    }

    /// Returns per-user `(user_id, cursor)` pairs with the batch-wide `limit` from their call, flattened across batches.
    pub async fn call_details(&self) -> Vec<(String, Option<u64>, u16)> {
        self.calls
            .lock()
            .await
            .iter()
            .flat_map(|call| {
                call.users
                    .iter()
                    .map(move |(user_id, cursor)| (user_id.clone(), *cursor, call.limit))
            })
            .collect()
    }

    /// Returns the raw batch calls, preserving batch structure.
    #[allow(dead_code)] // Used in PR 2+ batch tests
    pub async fn batch_calls(&self) -> Vec<FetchCall> {
        self.calls.lock().await.clone()
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

        // Extract the single-user ID before moving batch into the call record.
        let single_user_id = if batch.len() == 1 {
            Some(batch[0].0.clone())
        } else {
            None
        };

        self.calls.lock().await.push(FetchCall {
            users: batch,
            limit,
        });

        // For single-user fetches, check the user-keyed fixtures first.
        if let Some(user_id) = single_user_id {
            if let Some(events) = self.user_events.lock().await.remove(&user_id) {
                return events;
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
