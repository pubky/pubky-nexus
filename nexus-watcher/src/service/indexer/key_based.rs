use std::path::PathBuf;
use std::sync::Arc;

use nexus_common::models::event::EventProcessorError;
use nexus_common::models::homeserver::Homeserver;

use super::TEventProcessor;
use crate::events::Moderation;
use crate::service::user_hs_resolver;

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

    async fn run_internal(self: Arc<Self>) -> Result<(), EventProcessorError> {
        let current_hs_id = self.homeserver.id.to_string();

        // Find monitored user IDs from this HS
        let _user_ids = user_hs_resolver::get_user_ids_by_homeserver(&current_hs_id);

        // TODO Implement: fetch events per user

        Ok(())
    }
}
