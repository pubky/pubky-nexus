use nexus_common::types::DynError;
use nexus_common::DaemonConfig;
use nexus_watcher::NexusWatcherBuilder;
use nexus_webapi::{api_context::ApiContextBuilder, NexusApiBuilder};
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, path::PathBuf};
use tokio::try_join;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonLauncher {}

impl DaemonLauncher {
    pub async fn start(config_dir: PathBuf) -> Result<(), DynError> {
        let api_context = ApiContextBuilder::from_config_dir(config_dir.clone())
            .try_build()
            .await?;
        let nexus_webapi_builder = NexusApiBuilder(api_context);

        let config = DaemonConfig::read_or_create_config_file(config_dir).await?;
        let nexus_watcher_builder = NexusWatcherBuilder::with_stack(config.watcher, &config.stack);

        try_join!(nexus_webapi_builder.start(), nexus_watcher_builder.start())?;
        Ok(())
    }
}
