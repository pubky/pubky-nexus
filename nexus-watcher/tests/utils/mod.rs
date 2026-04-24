use nexus_common::models::event::{Event, EventProcessorError};
use nexus_watcher::events::{EventHandler, Moderation};
use pubky_app_specs::PubkyId;
use std::sync::Arc;

/// Mock implementation of EventHandler for testing.
///
/// `handle` returns `result` for every event.
pub struct MockEventHandler {
    pub result: Result<(), EventProcessorError>,
}

#[async_trait::async_trait]
impl EventHandler for MockEventHandler {
    async fn handle(&self, _event: &Event) -> Result<(), EventProcessorError> {
        self.result.clone()
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
