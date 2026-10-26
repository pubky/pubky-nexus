pub mod big_nexus;
mod constants;
pub mod inbound_bridge;
pub mod indexer;
pub mod runner;
pub mod scope_bootstrap;
pub mod scope_maintenance;
pub mod stats;
mod task_runner;
pub mod user_hs_resolver;

/// Module exports
pub use constants::{
    CATCH_UP_RUN_BUDGET, KEY_BASED_RUN_BUDGET, KEY_BASED_SLOW_RUN_MARGIN, SLOW_RUN_THRESHOLD,
    WATCHER_CONFIG_FILE_NAME,
};
pub use indexer::{
    HsEventProcessor, KeyBasedEventProcessor, RunError, RunOutcome, TEventProcessor,
};
pub use runner::{HsEventProcessorRunner, KeyBasedEventProcessorRunner, TEventProcessorRunner};
pub(crate) use task_runner::{run_periodic_tasks, PeriodicTask};
pub use user_hs_resolver::UserHsResolverRunner;

use crate::events::retry::{RetryProcessor, RetryScheduler};
use crate::events::{DefaultEventHandler, EventHandler};
use crate::homeserver::{HomeserverClient, PubkyHomeserverClient};
use crate::scope::{full_feed_homeservers, watcher_ingestor};
use crate::service::constants::DEFAULT_WATCHER_CONFIG_TOML;
use crate::service::inbound_bridge::InboundBridge;
use crate::service::scope_bootstrap::{bootstrap_seed_scope, NexusFollowing};
use crate::service::scope_maintenance::ScopeMaintainer;
use crate::service::task_runner::task_results_into_result;
use crate::NexusWatcherBuilder;
use nexus_common::db::PubkyConnector;
use nexus_common::file::ConfigLoader;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use nexus_common::WatcherConfig;
use nexus_common::{ScopeBootstrapConfig, ScopeConfig};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::watch::Receiver;
use tracing::{debug, info, warn};

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
    /// Currently runs up to six tasks, each on its own tick interval:
    /// 1. **Primary homeserver** ([`WatcherConfig::primary_hs_monitoring_interval_ms`]).
    /// 2. **External homeservers** ([`WatcherConfig::external_hs_monitoring_interval_ms`]).
    /// 3. **User HS resolver** ([`WatcherConfig::hs_resolver_interval_ms`]).
    /// 4. **Retry processor** ([`WatcherConfig::retry_processor_interval_ms`]).
    /// 5. **Scope maintenance** ([`ScopeMaintainer::tick_every_ms`]), unless the scope is
    ///    `everything`.
    /// 6. **Inbound bridge** ([`nexus_common::InboundBridgeConfig::interval_ms`]), when
    ///    configured for a seed scope.
    ///
    /// All tasks listen for the shutdown signal to exit gracefully. If any task panics,
    /// an internal cancellation signal is sent so that sibling tasks can finish their
    /// current iteration and exit.
    ///
    /// The tasks share one [`HomeserverClient`], one event handler and one retry scheduler.
    ///
    /// Expects [`PubkyConnector`] to be initialised.
    pub async fn start(shutdown_rx: Receiver<bool>, config: WatcherConfig) -> Result<(), DynError> {
        debug!(?config, "Running NexusWatcher with ");

        let primary_hs_monitoring_interval_ms = config.primary_hs_monitoring_interval_ms;
        let external_hs_monitoring_interval_ms = config.external_hs_monitoring_interval_ms;
        let hs_resolver_interval_ms = config.hs_resolver_interval_ms;
        let retry_processor_interval_ms = config.retry_processor_interval_ms;

        let client: Arc<dyn HomeserverClient> = Arc::new(PubkyHomeserverClient::from_config(
            PubkyConnector::get()?,
            &config,
        ));
        let scope_maintainer = Arc::new(ScopeMaintainer::from_config(&config, client.clone()));
        // Before the scope is prepared: a bridge configured without a seed scope stops the
        // start.
        let inbound_bridge = InboundBridge::from_config(&config, client.clone())?.map(Arc::new);
        prepare_scope(&config, client.clone(), &scope_maintainer).await?;
        let bootstrap = match (scope_maintainer.seed_scope(), &config.scope_bootstrap) {
            (Some(_), ScopeBootstrapConfig::Nexus { nexus_url }) => {
                // In the background: events are read meanwhile, and crawling finds the
                // members anyway if the big Nexus can't be read.
                let maintainer = scope_maintainer.clone();
                let source = NexusFollowing {
                    nexus_url: nexus_url.clone(),
                };
                Some(tokio::spawn(async move {
                    bootstrap_seed_scope(&maintainer, &source).await
                }))
            }
            _ => None,
        };

        let event_handler: Arc<dyn EventHandler> =
            Arc::new(DefaultEventHandler::from_config(&config, client.clone()));
        let retry_scheduler = Arc::new(RetryScheduler::from_config(&config));

        let hs_runner = Arc::new(HsEventProcessorRunner::new(
            &config,
            client.clone(),
            event_handler.clone(),
            retry_scheduler.clone(),
            shutdown_rx.clone(),
        ));
        let key_based_runner = Arc::new(KeyBasedEventProcessorRunner::new(
            &config,
            client.clone(),
            event_handler.clone(),
            retry_scheduler,
            shutdown_rx.clone(),
        ));
        let user_hs_resolver_runner = Arc::new(UserHsResolverRunner::from_config(
            &config,
            client,
            shutdown_rx.clone(),
        ));

        // Create retry processor
        let retry_processor = Arc::new(RetryProcessor::new(
            &config,
            event_handler,
            shutdown_rx.clone(),
        ));

        let mut tasks = vec![
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

        if scope_maintainer.is_periodic() {
            tasks.push(PeriodicTask::new(
                "scope-maintenance",
                scope_maintainer.tick_every_ms(),
                move || {
                    let maintainer = scope_maintainer.clone();
                    async move { maintainer.tick().await.map(|_| ()).map_err(DynError::from) }
                },
            ));
        }

        if let Some(bridge) = inbound_bridge {
            tasks.push(PeriodicTask::new(
                "inbound-bridge",
                config.inbound_bridge.interval_ms,
                move || {
                    let bridge = bridge.clone();
                    async move { bridge.poll().await }
                },
            ));
        }

        let task_results = stopping_after(bootstrap, run_periodic_tasks(tasks, shutdown_rx)).await;

        info!("Nexus Watcher shut down gracefully");
        task_results_into_result(task_results)
    }
}

/// Runs `run`, then stops `background` and waits for it to end, so nothing it started
/// outlives the watcher: a seed bootstrap would otherwise go on asking a big Nexus and
/// changing who is in scope after the watcher stopped. Stopping it midway is safe: an
/// admission cut short is finished by the next run.
async fn stopping_after<T>(
    background: Option<tokio::task::JoinHandle<()>>,
    run: impl Future<Output = T>,
) -> T {
    let output = run.await;
    if let Some(background) = background {
        background.abort();
        // Cancelled, or already done: either way it has ended.
        let _ = background.await;
    }
    output
}

/// Gets the scope ready before any event is read: the homeservers read whole are
/// persisted so their cursors exist, the moderator is made sure to be read, and who is in
/// scope is settled. Users' homeservers are looked up through `client`. Run by
/// [`NexusWatcher::start`]; public so tests can run it alone.
pub async fn prepare_scope(
    config: &WatcherConfig,
    client: Arc<dyn HomeserverClient>,
    maintainer: &ScopeMaintainer,
) -> Result<(), DynError> {
    info!(scope = ?config.stack.scope, "Indexing scope");

    for homeserver in full_feed_homeservers(config) {
        Homeserver::persist_if_unknown(homeserver).await?;
    }

    // The moderator's tags take content down, so a scoped Nexus must read it too.
    if config.stack.scope != ScopeConfig::Everything {
        watcher_ingestor(config, client)
            .ensure_in_scope(&config.moderation_id)
            .await?;
    }

    // Users of a narrower or wider previous scope moved out or back in, before anything
    // is polled.
    maintainer.at_start().await?;

    if let ScopeConfig::Homeservers { homeservers } = &config.stack.scope {
        let blacklist = &config.stack.net.external_hs_pk_blacklist;
        for listed in homeservers.iter().filter(|hs| blacklist.contains(hs)) {
            warn!(homeserver = %listed, "A listed homeserver is blacklisted: it is not read");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        stopping_after, NexusWatcher, DEFAULT_WATCHER_CONFIG_TOML, WATCHER_CONFIG_FILE_NAME,
    };
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    /// A missing config file is written from the default config, which loads
    /// with the trust filter off.
    #[tokio::test]
    async fn test_load_or_create_config_writes_the_default() {
        let dir = tempfile::tempdir().unwrap();

        let config = NexusWatcher::load_or_create_config(dir.path())
            .await
            .expect("the default config should load");
        assert!(!config.features.hide_unranked_authors);

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

    /// Background work still running when the watcher's tasks end is stopped before the
    /// watcher returns: nothing it would do later happens.
    #[tokio::test(start_paused = true)]
    async fn background_work_stops_with_the_watcher() {
        let done = Arc::new(AtomicBool::new(false));
        let background = tokio::spawn({
            let done = done.clone();
            async move {
                tokio::time::sleep(Duration::from_secs(60)).await;
                done.store(true, Ordering::SeqCst);
            }
        });

        let output = stopping_after(Some(background), async { "stopped" }).await;
        tokio::time::sleep(Duration::from_secs(120)).await;

        assert_eq!(output, "stopped");
        assert!(!done.load(Ordering::SeqCst), "stopped, not left running");
    }
}
