mod constants;
mod processor;
mod processor_runner;
mod stats;
mod traits;

/// Module exports
pub use constants::{PROCESSING_TIMEOUT_SECS, WATCHER_CONFIG_FILE_NAME};
pub use processor::EventProcessor;
pub use processor_runner::EventProcessorRunner;
pub use traits::{TEventProcessor, TEventProcessorRunner};

use crate::NexusWatcherBuilder;
use nexus_common::file::ConfigLoader;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use nexus_common::utils::create_shutdown_rx;
use nexus_common::{DaemonConfig, WatcherConfig};
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use std::sync::Arc;
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

    /// Starts the Nexus Watcher with 3 parallel loops:
    ///
    /// 1. **Default homeserver loop**: Processes events from the default homeserver defined in [`WatcherConfig`].
    /// 2. **Other homeservers loop**: Processes events from all other monitored homeservers, excluding the default.
    /// 3. **Reserved loop**: Placeholder for future use.
    ///
    /// All loops share the same tick interval ([`WatcherConfig::watcher_sleep`]) and listen for
    /// the shutdown signal to exit gracefully.
    pub async fn start(shutdown_rx: Receiver<bool>, config: WatcherConfig) -> Result<(), DynError> {
        debug!(?config, "Running NexusWatcher with ");

        let config_hs = PubkyId::try_from(config.homeserver.as_str())?;
        Homeserver::persist_if_unknown(config_hs).await?;

        let watcher_sleep = config.watcher_sleep;
        let ev_processor_runner = EventProcessorRunner::from_config(&config, shutdown_rx.clone());
        let ev_processor_runner = Arc::new(ev_processor_runner);

        // Thread 1: Default homeserver processing
        let default_hs_handle = {
            let runner = ev_processor_runner.clone();
            let mut shutdown = shutdown_rx.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_millis(watcher_sleep));
                loop {
                    tokio::select! {
                        _ = shutdown.changed() => {
                            info!("SIGINT received, exiting default homeserver loop");
                            break;
                        }
                        _ = interval.tick() => {
                            debug!("Indexing default homeserver…");
                            _ = runner
                                .run_default_homeserver()
                                .await
                                .inspect_err(|e| error!("Failed to run default homeserver event processor: {e}"));
                        }
                    }
                }
            })
        };

        // Thread 2: Other homeservers processing
        let other_hs_handle = {
            let runner = ev_processor_runner.clone();
            let mut shutdown = shutdown_rx.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_millis(watcher_sleep));
                loop {
                    tokio::select! {
                        _ = shutdown.changed() => {
                            info!("SIGINT received, exiting other homeservers loop");
                            break;
                        }
                        _ = interval.tick() => {
                            debug!("Indexing other homeservers…");
                            _ = runner
                                .run_all()
                                .await
                                .inspect_err(|e| error!("Failed to start event processors run: {e}"));
                        }
                    }
                }
            })
        };

        // Thread 3: Reserved for future use
        let reserved_handle = {
            let mut shutdown = shutdown_rx.clone();
            tokio::spawn(async move {
                // TODO: Reserved for future use
                let _ = shutdown.changed().await;
                info!("SIGINT received, exiting reserved loop");
            })
        };

        let _ = tokio::try_join!(default_hs_handle, other_hs_handle, reserved_handle);
        info!("Nexus Watcher shut down gracefully");
        Ok(())
    }
}
