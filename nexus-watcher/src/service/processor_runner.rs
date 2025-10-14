use crate::events::Moderation;
use crate::service::processor::EventProcessor;
use crate::service::traits::{TEventProcessor, TEventProcessorRunner};
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use nexus_common::WatcherConfig;
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch::Receiver;

pub struct EventProcessorRunner {
    pub limit: u32,
    pub files_path: PathBuf,
    pub tracer_name: String,
    pub moderation: Arc<Moderation>,
    pub shutdown_rx: Receiver<bool>,
    /// The default homeserver that the sync is done with
    pub default_homeserver: PubkyId,
}

impl EventProcessorRunner {
    /// Creates a new instance from the provided configuration
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
            default_homeserver: config.homeserver.clone(),
        }
    }
}

#[async_trait::async_trait]
impl TEventProcessorRunner for EventProcessorRunner {
    fn shutdown_rx(&self) -> Receiver<bool> {
        self.shutdown_rx.clone()
    }

    fn default_homeserver(&self) -> &str {
        &self.default_homeserver
    }

    async fn homeservers_by_priority(&self) -> Result<Vec<String>, DynError> {
        let mut hs_ids = Homeserver::get_all_from_graph().await?;

        // Move default homeserver to index 0 if it exists in the array to prioritize its processing
        if let Some(default_pos) = hs_ids
            .iter()
            .position(|hs_id| hs_id == self.default_homeserver())
        {
            let default_hs = hs_ids.remove(default_pos);
            hs_ids.insert(0, default_hs);
        }

        Ok(hs_ids)
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
