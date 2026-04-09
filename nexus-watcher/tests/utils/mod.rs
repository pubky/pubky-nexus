use nexus_common::models::event::{Event, EventProcessorError};
use nexus_watcher::events::{EventHandler, Moderation, TModeration};
use pubky_app_specs::PubkyId;
use std::sync::Arc;

/// Mock implementation of EventHandler for testing.
///
/// If `target_uri_substring` is set, `result` only applies to events whose URI contains
/// the substring; all other events return `Ok(())`. This lets tests sharing the Redis
/// retry queue target only their own events (e.g. by the unique post_id they use) and
/// leave any leftover events from parallel tests to be drained normally.
pub struct MockEventHandler {
    pub result: Result<(), EventProcessorError>,
    pub target_uri_substring: Option<String>,
    pub moderation: std::sync::Arc<dyn TModeration>,
}

#[async_trait::async_trait]
impl EventHandler for MockEventHandler {
    fn moderation(&self) -> &std::sync::Arc<dyn TModeration> {
        &self.moderation
    }

    async fn handle(&self, event: &Event) -> Result<(), EventProcessorError> {
        match &self.target_uri_substring {
            Some(s) if !event.uri.contains(s) => Ok(()),
            _ => self.result.clone(),
        }
    }
}

/// Default Moderation settings for tests
/// Returns the real Moderation implementation configured with test moderator ID and tags
pub fn default_moderation_tests() -> Arc<dyn TModeration> {
    // Moderator ID from moderator_key.pkarr (52-char z32 encoded ID without pubky prefix)
    let id = PubkyId::try_from("uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko")
        .expect("Hardcoded test moderation key should be valid");
    let tags = Vec::from(["label_to_moderate".to_string()]);
    Arc::new(Moderation { id, tags })
}
