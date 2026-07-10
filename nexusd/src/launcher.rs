use std::{fmt::Debug, path::PathBuf};

use nexus_common::DaemonConfig;
use nexus_common::{types::DynError, utils::create_shutdown_rx};
use nexus_watcher::NexusWatcherBuilder;
use nexus_webapi::{api_context::ApiContextBuilder, NexusApiBuilder};
use serde::{Deserialize, Serialize};
use tokio::{sync::watch::Receiver, try_join};

use crate::jobs::JobRegistry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonLauncher {}

impl DaemonLauncher {
    /// Starts a daemon, with separate threads for a [NexusApi], a [NexusWatcher]
    /// and the scheduled [jobs](crate::jobs).
    ///
    /// This is a blocking method. It only returns:
    /// - either when one of these services throws an error, or
    /// - when the shutdown signal is received and all services shut down
    ///
    /// ### Arguments
    ///
    /// - `config_dir`: the directory where the config file is expected to be
    /// - `job_registry`: the registry of available jobs
    /// - `shutdown_rx`: optional shutdown signal. If none is provided, a default one will be created, listening for Ctrl-C.
    pub async fn start(
        config_dir: PathBuf,
        job_registry: &JobRegistry,
        shutdown_rx: Option<Receiver<bool>>,
    ) -> Result<(), DynError> {
        let shutdown_rx = shutdown_rx.unwrap_or_else(create_shutdown_rx);

        let config = DaemonConfig::read_or_create_config_file(config_dir.clone()).await?;

        // Resolve + validate scheduled jobs before starting any service, so a
        // bad cron fails fast at startup.
        let jobs = job_registry.scheduled_jobs(&config)?;

        let api_context = ApiContextBuilder::from_config_dir(config_dir)
            .try_build()
            .await?;
        let nexus_webapi_builder = NexusApiBuilder::new(api_context);

        let nexus_watcher_builder = NexusWatcherBuilder::with_stack(config.watcher, &config.stack);

        try_join!(
            nexus_webapi_builder.start(Some(shutdown_rx.clone())),
            nexus_watcher_builder.start(Some(shutdown_rx.clone())),
            // Erase JobError to DynError so it unifies with the webapi/watcher
            // arms (try_join! needs one error type).
            async {
                crate::jobs::run(jobs, &config.stack, shutdown_rx)
                    .await
                    .map_err(DynError::from)
            },
        )?;
        Ok(())
    }
}
