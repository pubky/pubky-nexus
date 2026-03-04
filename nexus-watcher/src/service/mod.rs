mod constants;
mod processor;
mod processor_runner;
mod stats;
mod traits;

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

    /// Initializes configuration, persists the default homeserver, and delegates to [`Self::run_tasks`].
    pub async fn start(shutdown_rx: Receiver<bool>, config: WatcherConfig) -> Result<(), DynError> {
        debug!(?config, "Running NexusWatcher with ");

        let config_hs = PubkyId::try_from(config.homeserver.as_str())?;
        Homeserver::persist_if_unknown(config_hs).await?;

        let runner = EventProcessorRunner::from_config(&config, shutdown_rx.clone());
        Self::run_tasks(shutdown_rx, Arc::new(runner), config.watcher_sleep).await
    }

    /// Spawns processing tasks and waits for completion using a [`tokio::task::JoinSet`].
    ///
    /// Three parallel tasks are spawned:
    /// 1. **Default homeserver task**: calls [`TEventProcessorRunner::run_default_homeserver`] each tick.
    /// 2. **External homeservers task**: calls [`TEventProcessorRunner::run_external_homeservers`] each tick.
    ///
    /// When any task exits (normally or via panic), `JoinSet::join_next` returns and the
    /// remaining task is aborted if needed.
    ///
    /// Separated from [`Self::start`] to allow injection of mock runners in tests.
    pub async fn run_tasks(
        shutdown_rx: Receiver<bool>,
        runner: Arc<dyn TEventProcessorRunner>,
        watcher_sleep: u64,
    ) -> Result<(), DynError> {
        let mut set: tokio::task::JoinSet<&'static str> = tokio::task::JoinSet::new();

        // Task 1: Default homeserver processing
        let shutdown_rx_default = shutdown_rx.clone();
        let runner_default = runner.clone();
        set.spawn(async move {
            let mut shutdown_rx = shutdown_rx_default;
            let mut interval = tokio::time::interval(Duration::from_millis(watcher_sleep));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        info!("Shutdown received, exiting default homeserver loop");
                        break;
                    }
                    _ = interval.tick() => {
                        debug!("Indexing default homeserver…");
                        _ = runner_default
                            .run_default_homeserver()
                            .await
                            .inspect_err(|e| error!("Failed to run default homeserver event processor: {e}"));
                    }
                }
            }
            "default homeserver"
        });

        // Task 2: External homeservers processing
        let runner_external = runner;
        set.spawn(async move {
            let mut shutdown_rx = shutdown_rx;
            let mut interval = tokio::time::interval(Duration::from_millis(watcher_sleep));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        info!("Shutdown received, exiting external homeservers loop");
                        break;
                    }
                    _ = interval.tick() => {
                        debug!("Indexing external homeservers…");
                        _ = runner_external
                            .run_external_homeservers()
                            .await
                            .inspect_err(|e| error!("Failed to run external homeservers event processor: {e}"));
                    }
                }
            }
            "external homeservers"
        });

        // Block until the first task exits for any reason
        let mut had_error = false;

        match set.join_next().await {
            Some(Ok(label)) => info!("First task to exit: {label}"),
            Some(Err(e)) => {
                error!("Task failed (panic/cancel): {e}");
                had_error = true;
            }
            None => unreachable!("Expected at least one task in JoinSet"),
        }

        if had_error {
            set.abort_all();
        }

        // Drain remaining tasks
        while let Some(result) = set.join_next().await {
            match &result {
                Ok(label) => info!("Task exited: {label}"),
                Err(e) => {
                    error!("Task failed (panic/cancel): {e}");
                    had_error = true;
                }
            }
        }

        if had_error {
            return Err("Nexus Watcher stopped: one or more tasks failed".into());
        }

        info!("Nexus Watcher shut down gracefully");
        Ok(())
    }
}
