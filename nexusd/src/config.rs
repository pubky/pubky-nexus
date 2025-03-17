use async_trait::async_trait;
use nexus_api::{builder::NexusApiBuilder, Config as ApiConfig};
use nexus_common::{types::DynError, Config as StackConfig, ConfigLoader};
use nexus_watcher::{builder::NexusWatcherBuilder, Config as WatcherConfig};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, path::PathBuf};
use tracing::error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexusdConfig {
    api: ApiConfig,
    watcher: WatcherConfig,
    stack: StackConfig,
}

impl NexusdConfig {
    pub async fn load_builders_from_file(
        config_file: PathBuf,
    ) -> Result<(NexusApiBuilder, NexusWatcherBuilder), DynError> {
        let config: NexusdConfig = NexusdConfig::load(&config_file).await.map_err(|e| {
            error!("Failed to load config file {:?}: {}", config_file, e);
            e
        })?;
        Ok((
            NexusApiBuilder::with_stack(config.api, &config.stack),
            NexusWatcherBuilder::with_stack(config.watcher, &config.stack),
        ))
    }
}

#[async_trait]
impl<T> ConfigLoader<T> for NexusdConfig where T: DeserializeOwned + Send + Sync + Debug {}
