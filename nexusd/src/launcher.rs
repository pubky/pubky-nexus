use std::{fmt::Debug, path::PathBuf};

use nexus_common::DaemonConfig;
use nexus_common::{types::DynError, utils::create_shutdown_rx};
use nexus_watcher::NexusWatcherBuilder;
use nexus_webapi::{api_context::ApiContextBuilder, NexusApiBuilder};
use serde::{Deserialize, Serialize};
use tokio::{sync::watch::Receiver, try_join};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonLauncher {}

impl DaemonLauncher {
    /// Starts a daemon, with separate threads for a [NexusApi] and a [NexusWatcher] instances.
    ///
    /// This is a blocking method. It only returns:
    /// - either when one of these services throws an error, or
    /// - when the shutdown signal is received and both services shut down
    ///
    /// ### Arguments
    ///
    /// - `config_dir`: the directory where the config file is expected to be
    /// - `shutdown_rx`: optional shutdown signal. If none is provided, a default one will be created, listening for Ctrl-C.
    pub async fn start(
        config_dir: PathBuf,
        shutdown_rx: Option<Receiver<bool>>,
    ) -> Result<(), DynError> {
        let shutdown_rx = shutdown_rx.unwrap_or_else(create_shutdown_rx);

        let api_context = ApiContextBuilder::from_config_dir(config_dir.clone())
            .try_build()
            .await?;
        let nexus_webapi_builder = NexusApiBuilder::new(api_context);

        let config = DaemonConfig::read_or_create_config_file(config_dir).await?;
        let nexus_watcher_builder = NexusWatcherBuilder::with_stack(config.watcher, &config.stack);

        try_join!(
            nexus_webapi_builder.start(Some(shutdown_rx.clone())),
            nexus_watcher_builder.start(Some(shutdown_rx))
        )?;
        Ok(())
    }
}
