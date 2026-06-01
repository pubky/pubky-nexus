use std::collections::{HashMap, VecDeque};

use tokio::sync::Mutex;

use nexus_common::models::event::EventProcessorError;
use nexus_watcher::service::indexer::KeyBasedEventSource;
use pubky::{Event as StreamEvent, EventCursor, PublicKey};

type FetchEventsResult = Result<Vec<StreamEvent>, EventProcessorError>;

#[derive(Default)]
pub struct MockKeyBasedEventSource {
    /// Event batches returned in fetch order.
    /// Useful when user ordering is not important and tests only care about processor flow.
    events: Mutex<VecDeque<FetchEventsResult>>,

    /// Event batches returned by requested user ID.
    /// Useful when graph user ordering is intentionally not part of the assertion.
    user_events: Mutex<HashMap<String, FetchEventsResult>>,

    /// User IDs, cursors, and limits requested from the mock, in fetch order.
    /// Useful for asserting the processor continued to, or stopped before, specific users.
    calls: Mutex<Vec<(String, u64, u16)>>,
}

impl MockKeyBasedEventSource {
    pub async fn with_events(self, events: Vec<Vec<StreamEvent>>) -> Self {
        *self.events.lock().await = events.into_iter().map(Ok).collect();
        self
    }

    pub async fn with_results(self, results: Vec<FetchEventsResult>) -> Self {
        *self.events.lock().await = results.into();
        self
    }

    pub async fn with_user_events(self, events: Vec<(String, Vec<StreamEvent>)>) -> Self {
        *self.user_events.lock().await = events
            .into_iter()
            .map(|(user_id, events)| (user_id, Ok(events)))
            .collect();
        self
    }

    pub async fn with_user_results(self, results: Vec<(String, FetchEventsResult)>) -> Self {
        *self.user_events.lock().await = results.into_iter().collect();
        self
    }

    pub async fn calls(&self) -> Vec<String> {
        self.calls
            .lock()
            .await
            .iter()
            .map(|(user_id, _, _)| user_id.clone())
            .collect()
    }

    pub async fn call_details(&self) -> Vec<(String, u64, u16)> {
        self.calls.lock().await.clone()
    }
}

#[async_trait::async_trait]
impl KeyBasedEventSource for MockKeyBasedEventSource {
    async fn fetch_events(
        &self,
        _hs_pk: &PublicKey,
        user_pk: &PublicKey,
        cursor: EventCursor,
        limit: u16,
    ) -> Result<Vec<StreamEvent>, EventProcessorError> {
        let user_id = user_pk.z32();
        self.calls
            .lock()
            .await
            .push((user_id.clone(), cursor.id(), limit));

        if let Some(events) = self.user_events.lock().await.remove(&user_id) {
            return events;
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
