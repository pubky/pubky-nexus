use super::TEventProcessorRunner;
use crate::events::retry::RetryScheduler;
use crate::events::{DefaultEventHandler, EventHandler, Moderation, TModeration};
use crate::service::indexer::{HsEventProcessor, TEventProcessor};
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use nexus_common::WatcherConfig;
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch::Receiver;

pub struct HsEventProcessorRunner {
    /// See [WatcherConfig::events_limit]
    pub limit: u32,
    pub files_path: PathBuf,
    pub moderation: Arc<dyn TModeration>,
    pub event_handler: Arc<dyn EventHandler>,
    pub shutdown_rx: Receiver<bool>,
    /// See [WatcherConfig::homeserver]
    pub default_homeserver: PubkyId,
    /// Scheduler shared with every processor this runner builds
    pub retry_scheduler: Arc<RetryScheduler>,
}

impl HsEventProcessorRunner {
    /// Creates a new instance from the provided configuration
    pub fn from_config(config: &WatcherConfig, shutdown_rx: Receiver<bool>) -> Self {
        let moderation = Moderation::from_config(config);
        Self {
            limit: config.events_limit,
            files_path: config.stack.files_path.clone(),
            moderation: moderation.clone(),
            event_handler: Arc::new(DefaultEventHandler::new(moderation)),
            shutdown_rx,
            default_homeserver: config.homeserver.clone(),
            retry_scheduler: Arc::new(RetryScheduler::from_config(config)),
        }
    }

    pub fn default_homeserver(&self) -> &str {
        &self.default_homeserver
    }
}

#[async_trait::async_trait]
impl TEventProcessorRunner for HsEventProcessorRunner {
    fn shutdown_rx(&self) -> Receiver<bool> {
        self.shutdown_rx.clone()
    }

    /// Creates and returns a new event processor instance for the specified homeserver
    async fn build(&self, homeserver_id: String) -> Result<Arc<dyn TEventProcessor>, DynError> {
        let homeserver_id = PubkyId::try_from(&homeserver_id)?;
        let homeserver = Homeserver::get_by_id(homeserver_id)
            .await?
            .ok_or("Homeserver not found")?;

        Ok(Arc::new(HsEventProcessor {
            homeserver,
            limit: self.limit,
            files_path: self.files_path.clone(),
            moderation: self.moderation.clone(),
            event_handler: self.event_handler.clone(),
            shutdown_rx: self.shutdown_rx.clone(),
            retry_scheduler: self.retry_scheduler.clone(),
        }))
    }

    async fn pre_run(&self) -> Result<Vec<String>, DynError> {
        Ok(vec![self.default_homeserver.to_string()])
    }
}
