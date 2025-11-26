use crate::service::NexusWatcher;
use nexus_common::db::{DatabaseConfig, PubkyConnector};
use nexus_common::types::DynError;
use nexus_common::utils::create_shutdown_rx;
use nexus_common::{Level, StackConfig};
use nexus_common::{StackManager, WatcherConfig};
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use tokio::sync::watch::Receiver;

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
        let _ = PubkyConnector::initialise(testnet_host).await;
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
