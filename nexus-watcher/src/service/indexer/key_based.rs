use std::path::PathBuf;
use std::sync::Arc;

use nexus_common::models::event::EventProcessorError;
use nexus_common::models::homeserver::Homeserver;

use super::TEventProcessor;
use crate::events::retry::RetryScheduler;
use crate::events::{EventHandler, TModeration};
use crate::service::user_hs_resolver;

/// Event processor for non-default HSs, where the user-specific `/events-stream` endpoint is used
pub struct KeyBasedEventProcessor {
    /// The HS endpoint this processor fetches events from
    pub homeserver: Homeserver,

    pub files_path: PathBuf,
    pub moderation: Arc<dyn TModeration>,
    pub event_handler: Arc<dyn EventHandler>,
    /// Scheduler used to enqueue failed events onto the retry queue
    pub retry_scheduler: Arc<RetryScheduler>,
}

#[async_trait::async_trait]
impl TEventProcessor for KeyBasedEventProcessor {
    fn files_path(&self) -> &PathBuf {
        &self.files_path
    }

    fn moderation(&self) -> &Arc<dyn TModeration> {
        &self.moderation
    }

    fn event_handler(&self) -> &Arc<dyn EventHandler> {
        &self.event_handler
    }

    fn instance_name(&self) -> String {
        format!("KeyBasedEventProcessor with HS ID: {}", self.homeserver.id)
    }

    fn retry_scheduler(&self) -> &Arc<RetryScheduler> {
        &self.retry_scheduler
    }

    async fn run_internal(self: Arc<Self>) -> Result<(), EventProcessorError> {
        let current_hs_id = self.homeserver.id.to_string();

        // Find monitored user IDs from this HS
        let _user_ids = user_hs_resolver::get_user_ids_by_homeserver(&current_hs_id);

        // TODO Implement: fetch events per user

        Ok(())
    }
}
