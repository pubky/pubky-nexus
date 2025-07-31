use std::path::PathBuf;

use crate::events::processor::EventProcessor;

use nexus_common::db::{DatabaseConfig, PubkyClient};
use nexus_common::file::ConfigLoader;
use nexus_common::types::DynError;
use nexus_common::utils::create_shutdown_rx;
use nexus_common::{DaemonConfig, Level, StackConfig};
use nexus_common::{StackManager, WatcherConfig};
use pubky_app_specs::PubkyId;
use tokio::sync::watch::Receiver;
use tokio::time::Duration;
use tracing::{debug, error, info};

pub const WATCHER_CONFIG_FILE_NAME: &str = "watcher-config.toml";

#[derive(Debug, Default)]
pub struct NexusWatcherBuilder(pub WatcherConfig);

impl NexusWatcherBuilder {
    /// Creates a `NexusWatcherBuilder` instance with the given configuration and stack settings.
    pub fn with_stack(mut config: WatcherConfig, stack: &StackConfig) -> Self {
        config.stack = stack.clone();
        Self(config)
    }

    /// Sets the service name for observability (tracing, logging, monitoring)
    pub fn name(&mut self, name: String) -> &mut Self {
        self.0.name = name;

        self
    }

    /// Configures the logging level for the service, determining verbosity and log output
    pub fn log_level(&mut self, log_level: Level) -> &mut Self {
        self.0.stack.log_level = log_level;

        self
    }

    pub fn testnet(&mut self, testnet: bool) -> &mut Self {
        self.0.testnet = testnet;

        self
    }

    pub fn homeserver(&mut self, homeserver: PubkyId) -> &mut Self {
        self.0.homeserver = homeserver;

        self
    }

    /// Sets the directory for storing static files on the server
    pub fn files_path(&mut self, files_path: PathBuf) -> &mut Self {
        self.0.stack.files_path = files_path;

        self
    }

    /// Sets the OpenTelemetry endpoint for tracing and monitoring
    pub fn otlp_endpoint(&mut self, otlp_endpoint: Option<String>) -> &mut Self {
        self.0.stack.otlp_endpoint = otlp_endpoint;

        self
    }

    /// Sets the database configuration, including graph database and Redis settings
    pub fn db(&mut self, db: DatabaseConfig) -> &mut Self {
        self.0.stack.db = db;

        self
    }

    /// Opens ddbb connections and initialises tracing layer (if provided in config)
    pub async fn init_stack(&self) -> Result<(), DynError> {
        StackManager::setup(&self.0.name, &self.0.stack).await?;
        let testnet_host = if self.0.testnet {
            Some(self.0.testnet_host.as_str())
        } else {
            None
        };
        let _ = PubkyClient::initialise(testnet_host).await;
        Ok(())
    }

    /// Initializes the watcher integration test stack
    pub async fn init_test_stack(&self) -> Result<(), DynError> {
        StackManager::setup(&self.0.name, &self.0.stack).await?;
        Ok(())
    }

    /// Initializes the service stack and starts the NexusWatcher event loop
    ///
    /// ### Arguments
    ///
    /// - `shutdown_rx`: optional shutdown signal. If none is provided, a default one will be created, listening for Ctrl-C.
    pub async fn start(self, shutdown_rx: Option<Receiver<bool>>) -> Result<(), DynError> {
        let shutdown_rx = shutdown_rx.unwrap_or_else(create_shutdown_rx);

        self.init_stack().await?;
        NexusWatcher::start(shutdown_rx, self.0).await
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

    pub async fn start(
        mut shutdown_rx: Receiver<bool>,
        config: WatcherConfig,
    ) -> Result<(), DynError> {
        debug!(?config, "Running NexusWatcher with ");
        let mut event_processor = EventProcessor::from_config(&config).await?;

        let mut interval = tokio::time::interval(Duration::from_millis(config.watcher_sleep));

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    info!("SIGINT received, starting graceful shutdown...");
                    break;
                }
                _ = interval.tick() => {
                    info!("Fetching events…");
                    if let Err(e) = event_processor.run(shutdown_rx.clone()).await {
                        error!("Error while processing events: {:?}", e);
                    }
                }
            }
        }
        info!("Nexus Watcher shut down gracefully");
        Ok(())
    }
}
