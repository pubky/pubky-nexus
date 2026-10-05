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
use crate::service::constants::DEFAULT_WATCHER_CONFIG_TOML;
use crate::service::task_runner::task_results_into_result;
use crate::NexusWatcherBuilder;
use nexus_common::file::ConfigLoader;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use nexus_common::WatcherConfig;
use std::path::{Path, PathBuf};
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
    /// If the file is missing, the default config is written to it first. An invalid file is an error.
    ///
    /// ### Arguments
    ///
    /// - `config_dir`: the directory where the config file is expected to be
    /// - `shutdown_rx`: optional shutdown signal. If none is provided, a default one will be created, listening for Ctrl-C.
    pub async fn start_from_path(
        config_dir: PathBuf,
        shutdown_rx: Option<Receiver<bool>>,
    ) -> Result<(), DynError> {
        let config = Self::load_or_create_config(&config_dir).await?;
        NexusWatcherBuilder(config).start(shutdown_rx).await
    }

    /// Loads [WATCHER_CONFIG_FILE_NAME] from `config_dir`, first writing the default config to it if it's missing
    async fn load_or_create_config(config_dir: &Path) -> Result<WatcherConfig, DynError> {
        let config_file_path = config_dir.join(WATCHER_CONFIG_FILE_NAME);
        WatcherConfig::load_or_create(config_file_path, DEFAULT_WATCHER_CONFIG_TOML).await
    }

    /// Starts the Nexus Watcher with parallel periodic task loops.
    ///
    /// Currently runs four tasks, each on its own tick interval:
    /// 1. **Primary homeserver** ([`WatcherConfig::primary_hs_monitoring_interval_ms`]).
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

        let primary_hs_monitoring_interval_ms = config.primary_hs_monitoring_interval_ms;
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
                "primary-homeserver",
                primary_hs_monitoring_interval_ms,
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

#[cfg(test)]
mod tests {
    use super::{NexusWatcher, DEFAULT_WATCHER_CONFIG_TOML, WATCHER_CONFIG_FILE_NAME};

    /// A missing config file is written from the default config, which loads.
    #[tokio::test]
    async fn test_load_or_create_config_writes_the_default() {
        let dir = tempfile::tempdir().unwrap();

        NexusWatcher::load_or_create_config(dir.path())
            .await
            .expect("the default config should load");

        let written = std::fs::read_to_string(dir.path().join(WATCHER_CONFIG_FILE_NAME)).unwrap();
        assert_eq!(written, DEFAULT_WATCHER_CONFIG_TOML);
    }

    /// An invalid config file is an error. It's left as it is, and nothing else is written next to it.
    #[tokio::test]
    async fn test_start_from_path_rejects_an_invalid_config() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().to_path_buf();
        let config_file_path = config_dir.join(WATCHER_CONFIG_FILE_NAME);
        // Already signalled, so a start that wrongly succeeds returns instead of waiting for Ctrl-C
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        shutdown_tx.send(true).unwrap();

        std::fs::write(&config_file_path, "homeserver = 1").unwrap();
        let err = NexusWatcher::start_from_path(config_dir.clone(), Some(shutdown_rx))
            .await
            .expect_err("an invalid config file must be an error");
        assert!(err.to_string().contains(WATCHER_CONFIG_FILE_NAME), "{err}");

        assert_eq!(
            std::fs::read_to_string(&config_file_path).unwrap(),
            "homeserver = 1"
        );
        let files: Vec<_> = std::fs::read_dir(&config_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(
            files,
            [WATCHER_CONFIG_FILE_NAME],
            "nothing else may be written"
        );
    }
}
