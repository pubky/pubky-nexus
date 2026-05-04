use nexus_common::models::event::{Event, EventProcessorError};
use nexus_watcher::events::{EventHandler, Moderation};
use pubky_app_specs::PubkyId;
use std::sync::Arc;
use std::sync::Mutex;

/// Mock implementation of EventHandler for testing.
///
/// If `target_uri_substring` is set, `result` only applies to events whose URI contains
/// the substring; all other events return `Ok(())`.
///
/// In principle, some retry tests could be written as integration tests using [WatcherTest],
/// real local DHT homeservers, and real events. That would test more of the full pipeline.
/// However, [MockEventHandler] makes it possible to retry processor tests deterministically
/// force exact `handle()` outcomes, especially cases that are hard or flaky to create with real HSs.
pub struct MockEventHandler {
    pub result: Result<(), EventProcessorError>,
    pub target_uri_substring: Option<String>,
    /// Tracks how many times `handle()` was invoked. Shared via `Arc` so tests
    /// can read the count after processing.
    pub handle_count: Arc<Mutex<usize>>,
}

impl MockEventHandler {
    /// Returns the number of times `handle()` was called.
    pub fn get_handle_count(&self) -> usize {
        *self.handle_count.lock().unwrap()
    }
}

#[async_trait::async_trait]
impl EventHandler for MockEventHandler {
    async fn handle(&self, event: &Event) -> Result<(), EventProcessorError> {
        // Increment invocation counter on every call
        *self.handle_count.lock().unwrap() += 1;

        match &self.target_uri_substring {
            Some(s) if !event.uri.contains(s) => Ok(()),
            _ => self.result.clone(),
        }
    }
}

/// Default Moderation settings for tests
/// Returns the real Moderation implementation configured with test moderator ID and tags
pub fn default_moderation_tests() -> Arc<Moderation> {
    // Moderator ID from moderator_key.pkarr (52-char z32 encoded ID without pubky prefix)
    let id = PubkyId::try_from("uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko")
        .expect("Hardcoded test moderation key should be valid");
    let tags = Vec::from(["label_to_moderate".to_string()]);
    Arc::new(Moderation { id, tags })
}
