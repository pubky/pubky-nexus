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
pub use stats::ProcessedStats;
pub use traits::{TEventProcessor, TEventProcessorRunner};

use crate::NexusWatcherBuilder;
use nexus_common::file::ConfigLoader;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::utils::create_shutdown_rx;
use nexus_common::{DaemonConfig, WatcherConfig};
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch::{Receiver, Sender};
use tokio::time::Duration;
use tracing::{debug, error, info};

/// Sends a shutdown signal when dropped. Attach to each spawned task so that if any task
/// exits (normally or via panic), the remaining tasks are notified to stop gracefully.
struct ShutdownGuard(Option<Sender<bool>>);

impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        if let Some(tx) = self.0.take() {
            let _ = tx.send(true);
        }
    }
}

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

    /// Initializes configuration, persists the default homeserver, and delegates to [`Self::run_tasks`].
    pub async fn start(shutdown_rx: Receiver<bool>, config: WatcherConfig) -> Result<(), DynError> {
        debug!(?config, "Running NexusWatcher with ");

        let config_hs = PubkyId::try_from(config.homeserver.as_str())?;
        Homeserver::persist_if_unknown(config_hs).await?;

        let runner = EventProcessorRunner::from_config(&config, shutdown_rx.clone());
        Self::run_tasks(shutdown_rx, Arc::new(runner), config.watcher_sleep).await
    }

    /// Spawns processing tasks with [`ShutdownGuard`] protection and waits for completion.
    ///
    /// Three parallel tasks are spawned:
    /// 1. **Default homeserver task**: calls [`TEventProcessorRunner::run_default_homeserver`] each tick.
    /// 2. **External homeservers task**: calls [`TEventProcessorRunner::run_external_homeservers`] each tick.
    /// 3. **Shutdown forwarder task**: bridges the external `shutdown_rx` (e.g. SIGINT) into an internal channel.
    ///
    /// Each task holds a [`ShutdownGuard`]. If any task exits or panics, the guard drops and
    /// signals all remaining tasks to shut down gracefully via the internal channel.
    ///
    /// Separated from [`Self::start`] to allow injection of mock runners in tests.
    pub async fn run_tasks(
        shutdown_rx: Receiver<bool>,
        runner: Arc<dyn TEventProcessorRunner>,
        watcher_sleep: u64,
    ) -> Result<(), DynError> {
        // Internal channel: tasks signal each other via ShutdownGuard on exit/panic.
        // The forwarder bridges the external SIGINT into this channel.
        let (internal_shutdown_tx, internal_shutdown_rx) = tokio::sync::watch::channel(false);

        // Task 1: Default homeserver processing
        let default_hs_handle = {
            let runner = runner.clone();
            let mut shutdown = internal_shutdown_rx.clone();
            let guard = ShutdownGuard(Some(internal_shutdown_tx.clone()));
            tokio::spawn(async move {
                let _guard = guard;
                let mut interval = tokio::time::interval(Duration::from_millis(watcher_sleep));
                loop {
                    tokio::select! {
                        _ = shutdown.changed() => {
                            info!("Shutdown received, exiting default homeserver loop");
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

        // Task 2: External homeservers processing
        let external_hss_handle = {
            let runner = runner.clone();
            let mut shutdown = internal_shutdown_rx.clone();
            let guard = ShutdownGuard(Some(internal_shutdown_tx.clone()));
            tokio::spawn(async move {
                let _guard = guard;
                let mut interval = tokio::time::interval(Duration::from_millis(watcher_sleep));
                loop {
                    tokio::select! {
                        _ = shutdown.changed() => {
                            info!("Shutdown received, exiting external homeservers loop");
                            break;
                        }
                        _ = interval.tick() => {
                            debug!("Indexing external homeservers…");
                            _ = runner
                                .run_external_homeservers()
                                .await
                                .inspect_err(|e| error!("Failed to run external homeservers event processor: {e}"));
                        }
                    }
                }
            })
        };

        // Forwarder: bridges external SIGINT into the internal shutdown channel.
        // Also listens to the internal channel so it exits when a sibling task crashes.
        let forwarder_handle = {
            let mut external = shutdown_rx;
            let mut internal = internal_shutdown_rx.clone();
            let guard = ShutdownGuard(Some(internal_shutdown_tx.clone()));
            tokio::spawn(async move {
                let _guard = guard;
                tokio::select! {
                    _ = external.changed() => {
                        info!("SIGINT received, forwarding shutdown to watcher tasks");
                    }
                    _ = internal.changed() => {
                        info!("Internal shutdown received in forwarder, exiting");
                    }
                }
            })
        };

        // Drop the original sender so only task guards and the forwarder hold senders
        drop(internal_shutdown_tx);

        let (r1, r2, r3) = tokio::join!(default_hs_handle, external_hss_handle, forwarder_handle);

        if let Err(ref e) = r1 {
            error!("Default homeserver task failed: {e}");
        }
        if let Err(ref e) = r2 {
            error!("External homeservers task failed: {e}");
        }
        if let Err(ref e) = r3 {
            error!("Shutdown forwarder task failed: {e}");
        }

        if r1.is_err() || r2.is_err() || r3.is_err() {
            return Err("Nexus Watcher stopped: one or more tasks failed".into());
        }

        info!("Nexus Watcher shut down gracefully");
        Ok(())
    }
}
