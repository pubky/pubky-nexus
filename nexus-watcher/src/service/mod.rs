mod constants;
pub mod indexer;
pub mod runner;
pub mod stats;
mod task_runner;
pub mod user_hs_resolver;

/// Module exports
pub use constants::{PROCESSING_TIMEOUT_SECS, WATCHER_CONFIG_FILE_NAME};
pub use indexer::{HsEventProcessor, KeyBasedEventProcessor, RunError, TEventProcessor};
pub use runner::{HsEventProcessorRunner, KeyBasedEventProcessorRunner, TEventProcessorRunner};
pub(crate) use task_runner::{run_periodic_tasks, PeriodicTask};
pub use user_hs_resolver::UserHsResolverRunner;

use crate::events::retry::RetryProcessor;
use crate::service::task_runner::task_results_into_result;
use crate::NexusWatcherBuilder;
use nexus_common::file::ConfigLoader;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use nexus_common::utils::create_shutdown_rx;
use nexus_common::{DaemonConfig, WatcherConfig};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch::Receiver;
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

    /// Starts the Nexus Watcher with parallel periodic task loops.
    ///
    /// Currently runs four tasks, each on its own tick interval:
    /// 1. **Default homeserver** ([`WatcherConfig::default_hs_monitoring_interval_ms`]).
    /// 2. **External homeservers** ([`WatcherConfig::external_hs_monitoring_interval_ms`]).
    /// 3. **User HS resolver** ([`WatcherConfig::hs_resolver_interval_ms`]).
    /// 4. **Retry processor** ([`WatcherConfig::retry_processor_interval_ms`]).
    ///
    /// All tasks listen for the shutdown signal to exit gracefully. If any task panics,
    /// an internal cancellation signal is sent so that sibling tasks can finish their
    /// current iteration and exit.
    pub async fn start(shutdown_rx: Receiver<bool>, config: WatcherConfig) -> Result<(), DynError> {
        debug!(?config, "Running NexusWatcher with ");

        Homeserver::persist_if_unknown(config.homeserver.clone()).await?;

        let default_hs_monitoring_interval_ms = config.default_hs_monitoring_interval_ms;
        let external_hs_monitoring_interval_ms = config.external_hs_monitoring_interval_ms;
        let hs_resolver_interval_ms = config.hs_resolver_interval_ms;
        let retry_processor_interval_ms = config.retry_processor_interval_ms;

        let hs_runner = Arc::new(HsEventProcessorRunner::from_config(
            &config,
            shutdown_rx.clone(),
        ));
        let key_based_runner = Arc::new(KeyBasedEventProcessorRunner::from_config(
            &config,
            shutdown_rx.clone(),
        ));
        let user_hs_resolver_runner = Arc::new(UserHsResolverRunner::from_config(
            &config,
            Box::new(user_hs_resolver::PubkyConnectorResolver),
            shutdown_rx.clone(),
        ));

        // Create retry processor
        let retry_processor = Arc::new(RetryProcessor::new(&config, shutdown_rx.clone()));

        let tasks = vec![
            PeriodicTask::new(
                "default-homeserver",
                default_hs_monitoring_interval_ms,
                move || {
                    let runner = hs_runner.clone();
                    async move { runner.run().await.map(|_| ()) }
                },
            ),
            PeriodicTask::new(
                "external-homeservers",
                external_hs_monitoring_interval_ms,
                move || {
                    let runner = key_based_runner.clone();
                    async move { runner.run().await.map(|_| ()) }
                },
            ),
            PeriodicTask::new("user-hs-resolver", hs_resolver_interval_ms, move || {
                let runner = user_hs_resolver_runner.clone();
                async move { runner.run().await }
            }),
            PeriodicTask::new("retry-processor", retry_processor_interval_ms, move || {
                let processor = retry_processor.clone();
                async move { processor.run().await.map_err(DynError::from) }
            }),
        ];

        let task_results = run_periodic_tasks(tasks, shutdown_rx).await;

        info!("Nexus Watcher shut down gracefully");
        task_results_into_result(task_results)
    }
}
