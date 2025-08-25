mod cycle_processor;
mod rolling_window;
mod constants;

/// Module exports
pub use constants::{MAX_CONCURRENT, PROCESSING_TIMEOUT_SECS, WATCHER_CONFIG_FILE_NAME, ProcessResult};
pub use rolling_window::RollingWindow;
pub use cycle_processor::CycleProcessor;

use nexus_common::file::ConfigLoader;
use crate::events::processor::EventProcessorFactory;
use crate::NexusWatcherBuilder;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use nexus_common::utils::create_shutdown_rx;
use nexus_common::{DaemonConfig, WatcherConfig};
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch::Receiver;
use tokio::time::Duration;
use tracing::{debug, info};



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

        // Check if the configured homeserver is persisted in the graph
        let config_hs = PubkyId::try_from(config.homeserver.as_str())?;
        Homeserver::persist_if_unknown(config_hs).await?;

        let event_processor_factory = EventProcessorFactory::from_config(&config);
        let period = Duration::from_millis(config.watcher_sleep);

        let mut cycle_processor = CycleProcessor::new(
            Arc::new(event_processor_factory),
            shutdown_rx.clone(),
            period,
        );

        loop {
            // Process one complete cycle and get elapsed time
            let elapsed = cycle_processor.run_cycle().await?;

            // Check shutdown after cycle completion
            if *shutdown_rx.borrow() {
                info!(
                    "Shutdown requested after cycle {}; exiting.",
                    cycle_processor.cycle_count()
                );
                break;
            }

            // Pace the next cycle with actual elapsed time
            if !cycle_processor.pace_cycle(elapsed, &mut shutdown_rx).await {
                info!("Shutdown during pacing; exiting.");
                break;
            }
        }

        info!(
            total_cycles = cycle_processor.cycle_count(),
            "Watcher shut down gracefully"
        );

        Ok(())
    }
}