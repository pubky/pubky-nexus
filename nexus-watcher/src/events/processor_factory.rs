use crate::events::{EventProcessor, Moderation};
use crate::events::{TEventProcessor, TEventProcessorFactory};
use crate::service::PROCESSING_TIMEOUT_SECS;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use nexus_common::WatcherConfig;
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch::Receiver;

// TODO: Move to the service module. It seems to be out of place here.
/// This implements the creation logic for [`EventProcessor`] objects
pub struct EventProcessorFactory {
    pub limit: u32,
    pub files_path: PathBuf,
    pub tracer_name: String,
    pub moderation: Arc<Moderation>,
    pub shutdown_rx: Receiver<bool>,
}

impl EventProcessorFactory {
    /// Creates a new factory instance from the provided configuration
    pub fn from_config(config: &WatcherConfig, shutdown_rx: Receiver<bool>) -> Self {
        Self {
            limit: config.events_limit,
            files_path: config.stack.files_path.clone(),
            tracer_name: config.name.clone(),
            moderation: Arc::new(Moderation {
                id: config.moderation_id.clone(),
                tags: config.moderated_tags.clone(),
            }),
            shutdown_rx,
        }
    }
}

#[async_trait::async_trait]
impl TEventProcessorFactory for EventProcessorFactory {
    /// Returns the timeout for the event processor
    fn timeout(&self) -> Duration {
        // TODO: Set timeout maybe from the config file
        Duration::from_secs(PROCESSING_TIMEOUT_SECS)
    }

    fn shutdown_rx(&self) -> Receiver<bool> {
        self.shutdown_rx.clone()
    }

    /// Creates and returns a new event processor instance for the specified homeserver
    async fn build(&self, homeserver_id: String) -> Result<Arc<dyn TEventProcessor>, DynError> {
        let homeserver_id = PubkyId::try_from(&homeserver_id)?;
        let homeserver = Homeserver::get_by_id(homeserver_id)
            .await?
            .ok_or("Homeserver not found")?;

        // Create a new event processor instance with the specified homeserver
        Ok(Arc::new(EventProcessor {
            homeserver,
            limit: self.limit,
            files_path: self.files_path.clone(),
            tracer_name: self.tracer_name.clone(),
            moderation: self.moderation.clone(),
            shutdown_rx: self.shutdown_rx.clone(),
        }))
    }
}
