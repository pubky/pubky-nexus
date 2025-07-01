use nexus_common::file::ConfigReader;
use nexus_common::types::DynError;
use nexus_common::DaemonConfig;
use nexus_watcher::NexusWatcherBuilder;
use nexus_webapi::NexusApiBuilder;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, path::PathBuf};
use tokio::join;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonLauncher {}

impl DaemonLauncher {
    pub async fn start(config_dir: PathBuf) -> Result<(), DynError> {
        let config = DaemonConfig::read_config_file(config_dir).await?;
        let nexus_webapi_builder = NexusApiBuilder::with_stack(config.api, &config.stack);
        let nexus_watcher_builder = NexusWatcherBuilder::with_stack(config.watcher, &config.stack);
        let _ = join!(nexus_webapi_builder.start(), nexus_watcher_builder.start());
        Ok(())
    }
}
