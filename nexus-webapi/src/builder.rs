use crate::api_context::{ApiContext, ApiContextBuilder};
use crate::routes;

use std::net::TcpListener;
use std::time::Duration;
use std::{fmt::Debug, net::SocketAddr, path::PathBuf};

use axum_server::{Handle, Server};
use futures_util::TryFutureExt;
use nexus_common::db::DatabaseConfig;
use nexus_common::file::ConfigLoader;
use nexus_common::types::DynError;
use nexus_common::Level;
use nexus_common::{ApiConfig, StackManager};
use tokio::signal;
use tracing::{debug, error, info};

pub const API_CONFIG_FILE_NAME: &str = "api-config.toml";

#[derive(Debug)]
pub struct NexusApiBuilder(pub ApiContext);

impl NexusApiBuilder {
    /// Sets the service name for observability (tracing, logging, monitoring)
    pub fn name(mut self, name: String) -> Self {
        self.0.api_config.name = name;

        self
    }

    /// Configures the logging level for the service, determining verbosity and log output
    pub fn log_level(mut self, log_level: Level) -> Self {
        self.0.api_config.stack.log_level = log_level;

        self
    }

    /// Sets the server's listening address for incoming connections
    pub fn public_addr(mut self, addr: SocketAddr) -> Self {
        self.0.api_config.public_addr = addr;

        self
    }

    /// Sets the directory for storing static files on the server
    pub fn files_path(mut self, files_path: PathBuf) -> Self {
        self.0.api_config.stack.files_path = files_path;

        self
    }

    /// Sets the OpenTelemetry endpoint for tracing and monitoring
    pub fn otlp_endpoint(mut self, otlp_endpoint: Option<String>) -> Self {
        self.0.api_config.stack.otlp_endpoint = otlp_endpoint;

        self
    }

    /// Sets the database configuration, including graph database and Redis settings
    pub fn db(mut self, db: DatabaseConfig) -> Self {
        self.0.api_config.stack.db = db;

        self
    }

    /// Opens ddbb connections and initialises tracing layer (if provided in config)
    pub async fn init_stack(&self) -> Result<(), DynError> {
        StackManager::setup(&self.0.api_config.name, &self.0.api_config.stack).await
    }

    pub async fn start(self) -> Result<NexusApi, DynError> {
        self.init_stack()
            .await
            .inspect_err(|e| tracing::error!("Failed to initialize stack: {e}"))?;

        NexusApi::start(self.0)
            .await
            .inspect_err(|e| tracing::error!("Failed to start Nexus API: {e}"))
    }
}

pub struct NexusApi {
    pub socket: SocketAddr,
}

impl NexusApi {
    /// Loads the [ApiConfig] from [API_CONFIG_FILE_NAME] in the given path and starts the Nexus API.
    ///
    /// If no [ApiConfig] file is found, it defaults to [NexusApi::start_from_daemon].
    pub async fn start_from_path(config_dir: PathBuf) -> Result<Self, DynError> {
        match ApiConfig::load(config_dir.join(API_CONFIG_FILE_NAME)).await {
            Ok(api_config) => {
                let api_context = ApiContextBuilder::from_config_dir(config_dir)
                    .api_config(api_config)
                    .try_build()
                    .await?;

                NexusApiBuilder(api_context).start().await
            }
            Err(_) => NexusApi::start_from_daemon(config_dir).await,
        }
    }

    /// Loads the [ApiConfig] from the [DaemonConfig] in the given path and starts the Nexus API.
    pub async fn start_from_daemon(config_dir: PathBuf) -> Result<Self, DynError> {
        let api_context = ApiContextBuilder::from_config_dir(config_dir)
            .try_build()
            .await?;

        NexusApiBuilder(api_context).start().await
    }

    /// It sets up the necessary routes, binds to the specified address, and starts the Axum server
    pub async fn start(ctx: ApiContext) -> Result<Self, DynError> {
        // Create all the routes of the API
        let app = routes::routes(ctx.api_config.stack.files_path.clone());
        debug!(?ctx.api_config, "Running NexusAPI with config");

        let public_addr = ctx.api_config.public_addr;
        let listener = TcpListener::bind(public_addr)
            .inspect_err(|e| error!("Failed to bind to {public_addr:?}: {e}"))?;
        let local_addr = listener.local_addr().unwrap_or_else(|e| {
            panic!("Failed to get local address after binding: {e}");
        });
        info!("Nexus API listening on {local_addr}");

        // TODO Create server, init TLS, register shutdown handle
        info!("Nexus API listening on http://{}", ctx.keypair.public_key());

        // Create a shutdown handle
        let handle = Handle::new();

        // Spin up the server
        tokio::spawn(
            Server::from_tcp(listener)
                .handle(handle.clone())
                .serve(app.into_make_service())
                .inspect_err(|e| tracing::error!("Nexus API server error: {e}")),
        );

        // Spawn a task that waits for Ctrl+C and then tells the handle to shut down
        tokio::spawn(async move {
            signal::ctrl_c()
                .await
                .expect("Failed to hook up Ctrl+C handler");
            info!("SIGINT received, starting graceful shutdown...");
            handle.graceful_shutdown(Some(Duration::from_secs(30)));
        });

        Ok(NexusApi { socket: local_addr })
    }
}
