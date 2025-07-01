use crate::routes;
use axum_server::{Handle, Server};
use nexus_common::db::DatabaseConfig;
use nexus_common::file::ConfigReader;
use nexus_common::types::DynError;
use nexus_common::{ApiConfig, DaemonConfig, StackManager};
use nexus_common::{Level, StackConfig};
use std::time::Duration;
use std::{fmt::Debug, net::SocketAddr, path::PathBuf};
use tokio::net::TcpListener;
use tokio::signal;
use tracing::{debug, error, info};

#[derive(Debug, Default)]
pub struct NexusApiBuilder(pub ApiConfig);

impl NexusApiBuilder {
    /// Creates a `NexusWatcherBuilder` instance with the given configuration and stack settings.
    pub fn with_stack(mut config: ApiConfig, stack: &StackConfig) -> Self {
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
        StackManager::setup(&self.0.name, &self.0.stack).await?;
        Ok(())
    }

    pub async fn start(self) -> Result<(), DynError> {
        if let Err(e) = self.init_stack().await {
            tracing::error!("Failed to initialize stack: {}", e);
            return Err(e);
        }

        if let Err(e) = NexusApi::start(self.0, None).await {
            tracing::error!("Failed to start Nexus API: {}", e);
            return Err(e);
        }

        Ok(())
    }

    /// Nexus API server for integration tests
    pub async fn start_test(self, listener: TcpListener) -> Result<(), DynError> {
        NexusApi::start(self.0, Some(listener)).await
    }
}

pub struct NexusApi {}

impl NexusApi {
    /// Creates a new instance with default configuration
    pub fn builder() -> NexusApiBuilder {
        NexusApiBuilder::default()
    }

    /// Loads the configuration from a file and starts the Nexus API
    pub async fn start_from_path(config_dir: PathBuf) -> Result<(), DynError> {
        let config = ApiConfig::read_config_file(config_dir).await?;
        NexusApiBuilder(config).start().await
    }

    /// Loads the configuration from nexusd service and starts the Nexus API
    pub async fn start_from_daemon(config_dir: PathBuf) -> Result<(), DynError> {
        let config = DaemonConfig::read_config_file(config_dir).await?;
        NexusApiBuilder(Into::<ApiConfig>::into(config))
            .start()
            .await
    }

    /// It sets up the necessary routes, binds to the specified address (if no
    /// listener is provided), and starts the Axum server
    pub async fn start(config: ApiConfig, listener: Option<TcpListener>) -> Result<(), DynError> {
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
            panic!("Failed to get local address after binding: {e}");
        });
        info!("Listening on {:?}", addr);

        let std_listener = listener.into_std()?;

        // Create a shutdown handle
        let handle = Handle::new();

        let server = Server::from_tcp(std_listener)
            .handle(handle.clone()) // attach the handle
            .serve(app.into_make_service());

        // Spawn a task that waits for Ctrl+C and then tells the handle to shut down
        tokio::spawn(async move {
            signal::ctrl_c()
                .await
                .expect("Failed to hook up Ctrl+C handler");
            info!("SIGINT received, starting graceful shutdown...");
            handle.graceful_shutdown(Some(Duration::from_secs(30)));
        });
        // Spin up the server
        server.await.map_err(Into::into)
    }
}
