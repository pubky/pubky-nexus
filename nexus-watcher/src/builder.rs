use crate::events::processor::EventProcessor;
use nexus_common::db::{DatabaseConfig, PubkyClient};
use nexus_common::file::ConfigReader;
use nexus_common::types::DynError;
use nexus_common::{DaemonConfig, Level, StackConfig};
use nexus_common::{StackManager, WatcherConfig};
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use tokio::time::Duration;
use tokio::{pin, signal};
use tracing::{debug, error, info};

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
        let _ = PubkyClient::initialise(self.0.testnet).await;
        Ok(())
    }

    /// Initializes the watcher integration test stack
    pub async fn init_test_stack(&self) -> Result<(), DynError> {
        StackManager::setup(&self.0.name, &self.0.stack).await?;
        Ok(())
    }

    /// Initializes the service stack and starts the NexusWatcher event loop
    pub async fn start(self) -> Result<(), DynError> {
        self.init_stack().await?;
        NexusWatcher::start(self.0).await
    }
}

pub struct NexusWatcher {}

impl NexusWatcher {
    /// Creates a new instance with default configuration
    pub fn builder() -> NexusWatcherBuilder {
        NexusWatcherBuilder::default()
    }

    /// Loads the configuration from a file and starts the Watcher
    pub async fn start_from_path(config_dir: PathBuf) -> Result<(), DynError> {
        let config = WatcherConfig::read_config_file(config_dir).await?;
        NexusWatcherBuilder(config).start().await
    }

    /// Loads the configuration from nexusd service and starts the Watcher
    pub async fn start_from_daemon(config_dir: PathBuf) -> Result<(), DynError> {
        let config = DaemonConfig::read_config_file(config_dir).await?;
        NexusWatcherBuilder(Into::<WatcherConfig>::into(config))
            .start()
            .await
    }

    pub async fn start(config: WatcherConfig) -> Result<(), DynError> {
        debug!(?config, "Running NexusWatcher with ");
        let mut event_processor = EventProcessor::from_config(&config).await?;

        let shutdown_signal = signal::ctrl_c();
        pin!(shutdown_signal);
        // If we wanted to handle SIGTERM too
        // let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())?;
        // Now we only catch SIGINT

        let mut interval = tokio::time::interval(Duration::from_millis(config.watcher_sleep));

        // TODO: This lets you cancel the underlying future instead of waiting for it to complete
        // To achieve low-latency shutdown (i.e. abort in-flight processing immediately on Ctrl+C),
        // consider offloading `event_processor.run()` into its own cancellable Tokio task (or spawn_blocking thread),
        // keeping its `JoinHandle`, and invoking `handle.abort()` when the shutdown (ctlr + c) future resolves
        loop {
            tokio::select! {
                _ = &mut shutdown_signal => {
                    info!("SIGINT received, starting graceful shutdown...");
                    break;
                }
                _ = interval.tick() => {
                    info!("Fetching events…");
                    if let Err(e) = event_processor.run().await {
                        error!("Error while processing events: {:?}", e);
                    }
                }
            }
        }
        info!("service shut down gracefully");
        Ok(())
    }
}
