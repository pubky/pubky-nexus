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
    /// See [WatcherConfig::events_limit]
    pub limit: u32,
    /// See [WatcherConfig::monitored_homeservers_limit]
    pub monitored_homeservers_limit: usize,
    pub files_path: PathBuf,
    pub tracer_name: String,
    pub moderation: Arc<Moderation>,
    pub shutdown_rx: Receiver<bool>,
    /// See [WatcherConfig::homeserver]
    pub default_homeserver: PubkyId,
}

impl EventProcessorRunner {
    /// Creates a new instance from the provided configuration
    pub fn from_config(config: &WatcherConfig, shutdown_rx: Receiver<bool>) -> Self {
        Self {
            limit: config.events_limit,
            monitored_homeservers_limit: config.monitored_homeservers_limit,
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

    fn monitored_homeservers_limit(&self) -> usize {
        self.monitored_homeservers_limit
    }

    async fn homeservers_by_priority(&self) -> Result<Vec<String>, DynError> {
        let hs_ids = Homeserver::get_all_from_graph().await?;

        // Exclude the default homeserver from the list, as it is processed separately
        let hs_ids = hs_ids
            .into_iter()
            .filter(|hs_id| hs_id != self.default_homeserver())
            .collect();

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
