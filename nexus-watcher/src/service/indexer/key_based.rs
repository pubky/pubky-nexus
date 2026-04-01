use std::path::PathBuf;
use std::sync::Arc;

use nexus_common::models::event::{Event, EventProcessorError};
use nexus_common::models::homeserver::Homeserver;
use tracing::debug;

use super::TEventProcessor;
use crate::events::retry::event::RetryEvent;
use crate::events::Moderation;

pub struct KeyBasedEventProcessor {
    pub homeserver: Homeserver,
    pub files_path: PathBuf,
    pub tracer_name: String,
    pub moderation: Arc<Moderation>,
}

#[async_trait::async_trait]
impl TEventProcessor for KeyBasedEventProcessor {
    fn files_path(&self) -> &PathBuf {
        &self.files_path
    }

    fn tracer_name(&self) -> &str {
        &self.tracer_name
    }

    fn moderation(&self) -> &Arc<Moderation> {
        &self.moderation
    }

    fn extract_retry_event_info(
        &self,
        _event: &Event,
        _error: EventProcessorError,
    ) -> Option<(String, RetryEvent)> {
        None
    }

    async fn run_internal(self: Arc<Self>) -> Result<(), EventProcessorError> {
        debug!("KeyBasedEventProcessor running for {}", self.homeserver.id);
        Ok(())
    }
}
