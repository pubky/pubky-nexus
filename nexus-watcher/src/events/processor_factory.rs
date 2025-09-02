use std::path::PathBuf;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use nexus_common::WatcherConfig;
use pubky_app_specs::PubkyId;
use crate::traits::{TEventProcessor, TEventProcessorFactory};
use crate::events::{EventProcessor, Moderation};

/// This implements the creation logic for [`EventProcessor`] objects
pub struct EventProcessorFactory {
    pub limit: u32,
    pub files_path: PathBuf,
    pub tracer_name: String,
    pub moderation: Moderation,
}

impl EventProcessorFactory {
    /// Creates a new factory instance from the provided configuration
    pub fn from_config(config: &WatcherConfig) -> Self {
        Self {
            limit: config.events_limit,
            files_path: config.stack.files_path.clone(),
            tracer_name: config.name.clone(),
            moderation: Moderation {
                id: config.moderation_id.clone(),
                tags: config.moderated_tags.clone(),
            },
        }
    }
}

#[async_trait::async_trait]
impl TEventProcessorFactory for EventProcessorFactory {
    async fn build(&self, homeserver_id: String) -> Result<Box<dyn TEventProcessor>, DynError> {
        let homeserver_id = PubkyId::try_from(&homeserver_id).map_err(DynError::from)?;
        let homeserver = Homeserver::get_by_id(homeserver_id)
            .await?
            .ok_or("Homeserver not found")?;

        Ok(Box::new(EventProcessor {
            homeserver,
            limit: self.limit,
            files_path: self.files_path.clone(),
            tracer_name: self.tracer_name.clone(),
            moderation: self.moderation.clone(),
        }))
    }
}