use crate::{routes, Config};
use nexus_common::db::DatabaseConfig;
use nexus_common::types::DynError;
use nexus_common::StackManager;
use nexus_common::{Config as StackConfig, ConfigLoader, Level};
use std::{fmt::Debug, net::SocketAddr, path::PathBuf};
use tokio::net::TcpListener;
use tracing::{debug, error, info};

#[derive(Debug, Default)]
pub struct NexusApiBuilder(pub(crate) Config);

impl NexusApiBuilder {
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

    /// Sets the server's listening address for incoming connections
    pub fn public_addr(&mut self, addr: SocketAddr) -> &mut Self {
        self.0.public_addr = addr;

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
        StackManager::setup(&self.0.name, &self.0.stack).await;
        Ok(())
    }

    pub async fn run(self) -> Result<(), DynError> {
        if let Err(e) = self.init_stack().await {
            tracing::error!("Failed to initialize stack: {}", e);
            return Err(e);
        }

        if let Err(e) = NexusApi::run(self.0, None).await {
            tracing::error!("Failed to start Nexus API: {}", e);
            return Err(e);
        }

        Ok(())
    }

    /// Nexus API server for integration tests
    pub async fn run_test(self, listener: TcpListener) -> Result<(), DynError> {
        NexusApi::run(self.0, Some(listener)).await
    }
}

pub struct NexusApi {}

impl NexusApi {
    /// Creates a new instance with default configuration
    pub fn builder() -> NexusApiBuilder {
        NexusApiBuilder::default()
    }

    /// Loads the configuration from a file and starts the Nexus API
    pub async fn run_with_config_file(config_file: PathBuf) -> Result<(), DynError> {
        let config = Config::load(&config_file).await.map_err(|e| {
            error!("Failed to load config file {:?}: {}", config_file, e);
            e
        })?;
        NexusApiBuilder(config).run().await
    }

    /// It sets up the necessary routes, binds to the specified address (if no
    /// listener is provided), and starts the Axum server
    pub async fn run(config: Config, listener: Option<TcpListener>) -> Result<(), DynError> {
        // Create all the routes of the API
        let app = routes::routes(config.stack.files_path.clone());
        debug!(?config, "Running NexusAPI with");

        let listener = match listener {
            Some(l) => l,
            None => TcpListener::bind(config.public_addr).await.map_err(|e| {
                error!("Failed to bind to {:?}: {}", config.public_addr, e);
                e
            })?,
        };
        let addr = listener.local_addr().unwrap_or_else(|e| {
            panic!("Failed to get local address after binding: {}", e);
        });
        info!("Listening on {:?}", addr);

        // Start server
        axum::serve(listener, app.into_make_service()).await?;
        Ok(())
    }
}
