use std::{fmt::Debug, path::PathBuf};

use nexus_common::{types::DynError, utils::create_shutdown_rx};
use nexus_watcher::NexusWatcherBuilder;
use nexus_webapi::{api_context::ApiContextBuilder, NexusApiBuilder};
use serde::{Deserialize, Serialize};
use tokio::{sync::watch::Receiver, try_join};

use crate::config::DaemonConfig;
use crate::jobs::{run, JobRegistry};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonLauncher {}

impl DaemonLauncher {
    /// Starts a daemon, with separate threads for a [NexusApi](nexus_webapi::NexusApi),
    /// a [NexusWatcher](nexus_watcher::service::NexusWatcher) and the scheduled [jobs](crate::jobs).
    /// The API and the watcher share one Pubky client.
    ///
    /// This is a blocking method. It only returns:
    /// - either when one of these services throws an error, or
    /// - when the shutdown signal is received and all services shut down
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

        // Resolve + validate scheduled jobs before starting any service, so a
        // bad cron fails fast at startup.
        let config = DaemonConfig::read_or_create_config_file(config_dir.clone()).await?;
        let jobs = JobRegistry::catalog(&config.trust_rank).scheduled_jobs(&config)?;

        // One Pubky client for the process, shared by the API and the watcher. The watcher
        // cannot run without it, so the daemon does not start when it cannot be built.
        let nexus_watcher_builder = NexusWatcherBuilder(config.watcher_config());
        let pubky_client = nexus_watcher_builder.build_client()?;

        let api_context = ApiContextBuilder::new(config.api_config(), config_dir)
            .pubky_client(pubky_client.clone())
            .try_build()?;
        let nexus_webapi_builder = NexusApiBuilder::new(api_context);

        try_join!(
            nexus_webapi_builder.start(Some(shutdown_rx.clone())),
            nexus_watcher_builder.start_with_client(Some(shutdown_rx.clone()), pubky_client),
            // Erase JobError to DynError so it unifies with the webapi/watcher
            // arms (try_join! needs one error type).
            async {
                run(jobs, &config.stack, shutdown_rx)
                    .await
                    .map_err(DynError::from)
            },
        )?;
        Ok(())
    }

    /// Starts only a [NexusApi](nexus_webapi::NexusApi), configured by the `[api]` and `[stack]` sections.
    ///
    /// This is a blocking method. It only returns after the shutdown signal is received and the API shut down.
    ///
    /// ### Arguments
    ///
    /// - `config_dir`: the directory where the config file is expected to be
    /// - `shutdown_rx`: optional shutdown signal. If none is provided, a default one will be created, listening for Ctrl-C.
    pub async fn start_api(
        config_dir: PathBuf,
        shutdown_rx: Option<Receiver<bool>>,
    ) -> Result<(), DynError> {
        let config = DaemonConfig::read_or_create_config_file(config_dir.clone()).await?;
        let api_context = ApiContextBuilder::new(config.api_config(), config_dir).try_build()?;

        NexusApiBuilder::new(api_context).start(shutdown_rx).await?;
        Ok(())
    }

    /// Starts only a [NexusWatcher](nexus_watcher::service::NexusWatcher), configured by the `[watcher]`
    /// and `[stack]` sections.
    ///
    /// This is a blocking method. It only returns after the shutdown signal is received and the watcher shut down.
    ///
    /// ### Arguments
    ///
    /// - `config_dir`: the directory where the config file is expected to be
    /// - `shutdown_rx`: optional shutdown signal. If none is provided, a default one will be created, listening for Ctrl-C.
    pub async fn start_watcher(
        config_dir: PathBuf,
        shutdown_rx: Option<Receiver<bool>>,
    ) -> Result<(), DynError> {
        let config = DaemonConfig::read_or_create_config_file(config_dir).await?;

        NexusWatcherBuilder(config.watcher_config())
            .start(shutdown_rx)
            .await
    }
}
