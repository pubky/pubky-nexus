mod constants;
mod processor;
mod processor_runner;
mod stats;
mod task_runner;
mod traits;
pub mod user_hs_resolver;

/// Module exports
pub use constants::{PROCESSING_TIMEOUT_SECS, WATCHER_CONFIG_FILE_NAME};
pub use processor::EventProcessor;
pub use processor_runner::EventProcessorRunner;
pub(crate) use task_runner::{run_periodic_tasks, PeriodicTask};
pub use traits::{TEventProcessor, TEventProcessorRunner};

use crate::service::task_runner::task_results_into_result;
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
    /// Currently runs three tasks:
    /// 1. **Default homeserver**: Processes events from the default homeserver defined in [`WatcherConfig`].
    /// 2. **External homeservers**: Processes events from all external monitored homeservers, excluding the default.
    /// 3. **User HS resolver**: Resolves each user's homeserver and persists `HOSTED_BY` relationships.
    ///
    /// The event-processing tasks share the same tick interval ([`WatcherConfig::watcher_sleep`]),
    /// while the HS resolver uses its own interval (`hs_resolver_sleep`, default 10 s).
    /// All tasks listen for the shutdown signal to exit gracefully. If any task panics,
    /// an internal cancellation signal is sent so that sibling tasks can finish their
    /// current iteration and exit.
    pub async fn start(shutdown_rx: Receiver<bool>, config: WatcherConfig) -> Result<(), DynError> {
        debug!(?config, "Running NexusWatcher with ");

        let config_hs = PubkyId::try_from(config.homeserver.as_str())?;
        Homeserver::persist_if_unknown(config_hs).await?;

        let watcher_sleep = config.watcher_sleep;
        let hs_resolver_sleep: u64 = 10_000;
        let hs_resolver_ttl = config.hs_resolver_ttl;
        let ev_processor_runner = EventProcessorRunner::from_config(&config, shutdown_rx.clone());
        let ev_processor_runner = Arc::new(ev_processor_runner);

        let default_hs_runner = ev_processor_runner.clone();
        let external_hs_runner = ev_processor_runner.clone();

        let tasks = vec![
            PeriodicTask::new("default-homeserver", watcher_sleep, move || {
                let runner = default_hs_runner.clone();
                async move { runner.run_default_homeserver().await.map(|_| ()) }
            }),
            PeriodicTask::new("external-homeservers", watcher_sleep, move || {
                let runner = external_hs_runner.clone();
                async move { runner.run_external_homeservers().await.map(|_| ()) }
            }),
            PeriodicTask::new("user-hs-resolver", hs_resolver_sleep, move || {
                async move { user_hs_resolver::run(hs_resolver_ttl).await }
            }),
        ];

        let task_results = run_periodic_tasks(tasks, shutdown_rx).await;

        info!("Nexus Watcher shut down gracefully");
        task_results_into_result(task_results)
    }
}
