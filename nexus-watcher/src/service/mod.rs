mod constants;
mod processor;
mod processor_runner;
mod stats;
mod traits;

/// Module exports
pub use constants::{PROCESSING_TIMEOUT_SECS, WATCHER_CONFIG_FILE_NAME};
use nexus_common::types::DynError;
pub use processor::EventProcessor;
pub use processor_runner::EventProcessorRunner;
pub use traits::{TEventProcessor, TEventProcessorRunner};

use crate::NexusWatcherBuilder;
use nexus_common::file::ConfigLoader;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::utils::create_shutdown_rx;
use nexus_common::{DaemonConfig, WatcherConfig};
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use tokio::sync::watch::Receiver;
use tokio::time::Duration;
use tracing::{debug, error, info};

pub struct NexusWatcher {}

impl NexusWatcher {
    /// Creates a new instance with default configuration
    pub fn builder() -> NexusWatcherBuilder {
        NexusWatcherBuilder::default()
    }

    /// Loads the [WatcherConfig] from [WATCHER_CONFIG_FILE_NAME] in the given path and starts the Nexus Watcher.
    ///
    /// If no [WatcherConfig] file is found, it defaults to [NexusWatcher::start_from_daemon].
    ///
    /// ### Arguments
    ///
    /// - `config_dir`: the directory where the config file is expected to be
    /// - `shutdown_rx`: optional shutdown signal. If none is provided, a default one will be created, listening for Ctrl-C.
    pub async fn start_from_path(
        config_dir: PathBuf,
        shutdown_rx: Option<Receiver<bool>>,
    ) -> Result<(), DynError> {
        let shutdown_rx = shutdown_rx.unwrap_or_else(create_shutdown_rx);

        match WatcherConfig::load(config_dir.join(WATCHER_CONFIG_FILE_NAME)).await {
            Ok(config) => NexusWatcherBuilder(config).start(Some(shutdown_rx)).await,
            Err(_) => NexusWatcher::start_from_daemon(config_dir, Some(shutdown_rx)).await,
        }
    }

    /// Derives the [WatcherConfig] from [DaemonConfig] (nexusd service config), loads it and starts the Watcher.
    ///
    /// If a [DaemonConfig] is not found, a new one is created in the given path with the default contents.
    ///
    /// ### Arguments
    ///
    /// - `config_dir`: the directory where the config file is expected to be
    /// - `shutdown_rx`: optional shutdown signal. If none is provided, a default one will be created, listening for Ctrl-C.
    pub async fn start_from_daemon(
        config_dir: PathBuf,
        shutdown_rx: Option<Receiver<bool>>,
    ) -> Result<(), DynError> {
        let daemon_config = DaemonConfig::read_or_create_config_file(config_dir).await?;
        let watcher_config = WatcherConfig::from(daemon_config);
        NexusWatcherBuilder(watcher_config).start(shutdown_rx).await
    }

    pub async fn start(
        mut shutdown_rx: Receiver<bool>,
        config: WatcherConfig,
    ) -> Result<(), DynError> {
        debug!(?config, "Running NexusWatcher with ");

        let config_hs = PubkyId::try_from(config.homeserver.as_str())?;
        Homeserver::persist_if_unknown(config_hs).await?;

        let mut interval = tokio::time::interval(Duration::from_millis(config.watcher_sleep));
        let ev_processor_runner = EventProcessorRunner::from_config(&config, shutdown_rx.clone());

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    info!("SIGINT received, exiting Nexus Watcher loop");
                    break;
                }
                _ = interval.tick() => {
                    debug!("Indexing homeservers…");
                    _ = ev_processor_runner
                        .run_all()
                        .await
                        .inspect_err(|e| error!("Failed to start event processors run: {e}"));
                }
            }
        }
        info!("Nexus Watcher shut down gracefully");
        Ok(())
    }
}
