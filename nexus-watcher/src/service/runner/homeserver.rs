use super::TEventProcessorRunner;
use crate::events::Moderation;
use crate::service::indexer::{HsEventProcessor, TEventProcessor};
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use nexus_common::{WatcherConfig, MAX_EVENTS_LIMIT};
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch::Receiver;
use tracing::warn;

pub struct HsEventProcessorRunner {
    /// See [WatcherConfig::events_limit]
    pub limit: u16,
    pub files_path: PathBuf,
    pub tracer_name: String,
    pub moderation: Arc<Moderation>,
    pub shutdown_rx: Receiver<bool>,
    /// See [WatcherConfig::homeserver]
    pub default_homeserver: PubkyId,
}

impl HsEventProcessorRunner {
    /// Creates a new instance from the provided configuration
    pub fn from_config(config: &WatcherConfig, shutdown_rx: Receiver<bool>) -> Self {
        let limit = config.events_limit.min(MAX_EVENTS_LIMIT);
        if config.events_limit > MAX_EVENTS_LIMIT {
            warn!(
                "events_limit ({}) exceeds max ({}), clamped",
                config.events_limit, MAX_EVENTS_LIMIT
            );
        }

        Self {
            limit,
            files_path: config.stack.files_path.clone(),
            tracer_name: config.stack.otlp.name.clone(),
            moderation: Moderation::from_config(config),
            shutdown_rx,
            default_homeserver: config.homeserver.clone(),
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
            tracer_name: self.tracer_name.clone(),
            moderation: self.moderation.clone(),
            shutdown_rx: self.shutdown_rx.clone(),
        }))
    }

    async fn pre_run(&self) -> Result<Vec<String>, DynError> {
        Ok(vec![self.default_homeserver.to_string()])
    }
}
