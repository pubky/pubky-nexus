use std::path::PathBuf;
use std::sync::Arc;

use nexus_common::models::event::EventProcessorError;
use nexus_common::models::homeserver::Homeserver;

use super::TEventProcessor;
use crate::events::Moderation;

/// Event processor for non-default HSs, where the user-specific `/events-stream` endpoint is used
pub struct KeyBasedEventProcessor {
    /// The HS endpoint this processor fetches events from
    /// TODO Used in X1 (see mod.rs)
    pub homeserver: Homeserver,

    pub files_path: PathBuf,
    pub moderation: Arc<Moderation>,
}

#[async_trait::async_trait]
impl TEventProcessor for KeyBasedEventProcessor {
    fn files_path(&self) -> &PathBuf {
        &self.files_path
    }

    fn moderation(&self) -> &Arc<Moderation> {
        &self.moderation
    }

    // TODO Implement
    async fn run_internal(self: Arc<Self>) -> Result<(), EventProcessorError> {
        Ok(())
    }
}
