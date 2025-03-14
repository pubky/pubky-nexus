use crate::events::processor::EventProcessor;
use crate::Config;
use nexus_common::db::{DatabaseConfig, PubkyClient};
use nexus_common::types::DynError;
use nexus_common::StackManager;
use nexus_common::{Config as StackConfig, ConfigLoader, Level};
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info};

#[derive(Debug, Default)]
pub struct NexusWatcherBuilder(pub(crate) Config);

impl NexusWatcherBuilder {
    /// Creates a `NexusWatcherBuilder` instance with the given configuration and stack settings.
    pub fn with_stack(mut config: Config, stack: &StackConfig) -> Self {
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
    pub async fn init_stack(&self) {
        StackManager::setup(&self.0.name, &self.0.stack).await;
        let _ = PubkyClient::initialise(self.0.testnet).await;
    }

    /// Initializes the watcher integration test stack
    pub async fn init_test_stack(&self) {
        StackManager::setup(&self.0.name, &self.0.stack).await;
    }

    /// Initializes the service stack and starts the NexusWatcher event loop
    pub async fn run(self) -> Result<(), DynError> {
        self.init_stack().await;
        NexusWatcher::run(self.0).await
    }
}

pub struct NexusWatcher {}

impl NexusWatcher {
    pub fn builder() -> NexusWatcherBuilder {
        NexusWatcherBuilder::default()
    }

    pub async fn run_with_config_file(config_file: PathBuf) -> Result<(), DynError> {
        let config = Config::load(&config_file).await.map_err(|e| {
            error!("Failed to load config file {:?}: {}", config_file, e);
            e
        })?;
        NexusWatcherBuilder(config).run().await
    }

    pub async fn run(config: Config) -> Result<(), DynError> {
        debug!(?config, "Running NexusWatcher with ");
        let mut event_processor = EventProcessor::from_config(&config).await?;

        loop {
            info!("Fetching events...");
            if let Err(e) = event_processor.run().await {
                error!("Uncaught error occurred while processing events: {:?}", e);
            }
            // Wait for X milliseconds before fetching events again
            sleep(Duration::from_millis(config.watcher_sleep)).await;
        }
    }
}
