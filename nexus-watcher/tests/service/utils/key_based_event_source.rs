use std::collections::{HashMap, VecDeque};

use tokio::sync::Mutex;

use nexus_common::models::event::EventProcessorError;
use nexus_watcher::service::indexer::{KeyBasedEventSource, UserNotFoundBackoff};
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

    /// Error returned on every fetch, without being consumed.
    /// Useful for simulating a persistently failing user (e.g. repeated 404s) across runs.
    sticky_error: Mutex<Option<EventProcessorError>>,

    /// User IDs, cursors, and limits requested from the mock, in fetch order.
    /// Useful for asserting the processor continued to, or stopped before, specific users.
    calls: Mutex<Vec<(String, u64, u16)>>,

    /// Real 404 backoff state, so processor skip behavior can be exercised in tests.
    user_not_found_backoff: UserNotFoundBackoff,
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

    /// Returns the given error on every fetch, without consuming it, so a user
    /// can be made to fail persistently across multiple processor runs.
    pub async fn with_sticky_error(self, error: EventProcessorError) -> Self {
        *self.sticky_error.lock().await = Some(error);
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

        if let Some(error) = self.sticky_error.lock().await.as_ref() {
            return Err(error.clone());
        }

        if let Some(events) = self.user_events.lock().await.remove(&user_id) {
            return events;
        }

        self.events
            .lock()
            .await
            .pop_front()
            .unwrap_or_else(|| Ok(Vec::new()))
    }

    fn should_skip_user(&self, user_pk: &PublicKey) -> bool {
        self.user_not_found_backoff.should_skip(user_pk)
    }

    fn record_user_not_found(&self, user_pk: &PublicKey) {
        self.user_not_found_backoff.record_not_found(user_pk);
    }

    fn clear_user_not_found(&self, user_pk: &PublicKey) {
        self.user_not_found_backoff.clear(user_pk);
    }
}
