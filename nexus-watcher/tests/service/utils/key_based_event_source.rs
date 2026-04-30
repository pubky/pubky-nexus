use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use nexus_common::models::event::EventProcessorError;
use nexus_watcher::service::indexer::KeyBasedEventSource;
use pubky::{Event as StreamEvent, EventCursor, PublicKey};

#[derive(Default)]
pub struct MockKeyBasedEventSource {
    /// Event batches returned in fetch order.
    /// Useful when user ordering is not important and tests only care about processor flow.
    events: Mutex<VecDeque<Vec<StreamEvent>>>,

    /// Event batches returned by requested user ID.
    /// Useful when graph user ordering is intentionally not part of the assertion.
    user_events: Mutex<HashMap<String, Vec<StreamEvent>>>,

    /// User IDs requested from the mock, in fetch order.
    /// Useful for asserting the processor continued to, or stopped before, specific users.
    calls: Mutex<Vec<String>>,
}

impl MockKeyBasedEventSource {
    pub fn with_events(self, events: Vec<Vec<StreamEvent>>) -> Self {
        *self.events.lock().unwrap() = events.into();
        self
    }

    pub fn with_user_events(self, events: Vec<(String, Vec<StreamEvent>)>) -> Self {
        *self.user_events.lock().unwrap() = events.into_iter().collect();
        self
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl KeyBasedEventSource for MockKeyBasedEventSource {
    async fn fetch_events(
        &self,
        _hs_pk: &PublicKey,
        user_pk: &PublicKey,
        _cursor: EventCursor,
        _limit: u16,
    ) -> Result<Vec<StreamEvent>, EventProcessorError> {
        let user_id = user_pk.z32();
        self.calls.lock().unwrap().push(user_id.clone());

        if let Some(events) = self.user_events.lock().unwrap().remove(&user_id) {
            return Ok(events);
        }

        Ok(self.events.lock().unwrap().pop_front().unwrap_or_default())
    }
}
