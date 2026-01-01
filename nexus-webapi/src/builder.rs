use crate::api_context::{ApiContext, ApiContextBuilder};
use crate::key_republisher::{KeyRepublisher, KeyRepublisherContext};
use crate::routes;

use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;
use std::{fmt::Debug, net::SocketAddr, path::PathBuf};

use axum::Router;
use axum_server::tls_rustls::{RustlsAcceptor, RustlsConfig};
use axum_server::Handle;
use futures_util::TryFutureExt;
use nexus_common::db::DatabaseConfig;
use nexus_common::file::ConfigLoader;
use nexus_common::types::DynError;
use nexus_common::utils::create_shutdown_rx;
use nexus_common::Level;
use nexus_common::{ApiConfig, StackManager};
use pkarr::{Keypair, PublicKey};
use tokio::sync::watch::Receiver;
use tracing::{debug, error, info};

pub const API_CONFIG_FILE_NAME: &str = "api-config.toml";

type ServerHandle = Handle<SocketAddr>;

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

    /// Creates and starts a [NexusApi] instance.
    ///
    /// This method is blocking and only returns after the shutdown signal is received and the [NexusApi] shut down.
    ///
    /// ### Arguments
    ///
    /// - `shutdown_rx`: optional shutdown signal. If none is provided, a default one will be created, listening for Ctrl-C.
    pub async fn start(self, shutdown_rx: Option<Receiver<bool>>) -> Result<NexusApi, DynError> {
        let mut shutdown_rx = shutdown_rx.unwrap_or_else(create_shutdown_rx);

        self.init_stack()
            .await
            .inspect_err(|e| error!("Failed to initialize stack: {e}"))?;

        let nexus_api = NexusApi::start(self.0)
            .await
            .inspect_err(|e| error!("Failed to start Nexus API: {e}"))?;

        info!("Nexus API HTTP: {}", nexus_api.icann_http_url());
        info!("Nexus API Pubky TLS: {}", nexus_api.pubky_tls_dns_url());
        info!("Nexus API Pubky TLS: {}", nexus_api.pubky_tls_ip_url());

        let _ = shutdown_rx.changed().await;
        info!("Received shutdown signal");

        Ok(nexus_api)
    }
}

pub struct NexusApi {
    ctx: ApiContext,

    /// Local socket address used for the interface exposed via ICANN DNS
    icann_http_socket: SocketAddr,
    icann_http_handle: ServerHandle,

    /// Local socket address used for the interface exposed via Pubky PKDNS
    pubky_tls_socket: SocketAddr,
    pubky_tls_handle: ServerHandle,

    #[allow(dead_code)]
    // Keep this alive. Republishing is stopped when the instance is dropped.
    key_republisher: KeyRepublisher,
}

impl NexusApi {
    /// Loads the [ApiConfig] from [API_CONFIG_FILE_NAME] in the given path and starts the Nexus API.
    ///
    /// If no [ApiConfig] file is found, it defaults to [NexusApi::start_from_daemon].
    ///
    /// ### Arguments
    ///
    /// - `config_dir`: the directory where the config file is expected to be
    /// - `shutdown_rx`: optional shutdown signal. If none is provided, a default one will be created, listening for Ctrl-C.
    pub async fn start_from_path(
        config_dir: PathBuf,
        shutdown_rx: Option<Receiver<bool>>,
    ) -> Result<Self, DynError> {
        match ApiConfig::load(config_dir.join(API_CONFIG_FILE_NAME)).await {
            Ok(api_config) => {
                let api_context = ApiContextBuilder::from_config_dir(config_dir)
                    .api_config(api_config)
                    .try_build()
                    .await?;

                NexusApiBuilder(api_context).start(shutdown_rx).await
            }
            Err(_) => NexusApi::start_from_daemon(config_dir, shutdown_rx).await,
        }
    }

    /// Loads the [ApiConfig] from the [DaemonConfig] in the given path and starts the Nexus API.
    ///
    /// ### Arguments
    ///
    /// - `config_dir`: the directory where the config file is expected to be
    /// - `shutdown_rx`: optional shutdown signal. If none is provided, a default one will be created, listening for Ctrl-C.
    pub async fn start_from_daemon(
        config_dir: PathBuf,
        shutdown_rx: Option<Receiver<bool>>,
    ) -> Result<Self, DynError> {
        let shutdown_rx = shutdown_rx.unwrap_or_else(create_shutdown_rx);

        let api_context = ApiContextBuilder::from_config_dir(config_dir)
            .try_build()
            .await?;

        NexusApiBuilder(api_context).start(Some(shutdown_rx)).await
    }

    /// It sets up the necessary routes, binds to the specified address, and starts the Axum server
    pub async fn start(ctx: ApiContext) -> Result<Self, DynError> {
        // Create all the routes of the API
        let router = routes::routes(ctx.api_config.stack.files_path.clone());
        debug!(?ctx.api_config, "Running NexusAPI with config");

        let (icann_http_handle, icann_http_socket) =
            Self::start_icann_http_server(&ctx, router.clone()).await?;

        let (pubky_tls_handle, pubky_tls_socket) =
            Self::start_pubky_tls_server(&ctx, router).await?;

        let ks_ctx = derive_key_publisher_context(&ctx, pubky_tls_socket.port());
        let key_republisher = KeyRepublisher::start(&ks_ctx).await?;

        Ok(NexusApi {
            ctx,
            icann_http_socket,
            icann_http_handle,
            pubky_tls_socket,
            pubky_tls_handle,
            key_republisher,
        })
    }

    async fn start_icann_http_server(
        ctx: &ApiContext,
        router: Router,
    ) -> Result<(ServerHandle, SocketAddr), DynError> {
        let public_addr = ctx.api_config.public_addr;
        let listener = TcpListener::bind(public_addr)
            .inspect_err(|e| error!("Failed to bind to {public_addr:?}: {e}"))?;
        listener
            .set_nonblocking(true)
            .inspect_err(|e| error!("Failed to set {public_addr:?} listener non-blocking: {e}"))?;
        let local_addr = listener
            .local_addr()
            .inspect_err(|e| error!("Failed to get local address after binding: {e})"))?;
        let handle = ServerHandle::new();

        let icann_server = axum_server::from_tcp(listener).inspect_err(|e| {
            error!("Failed to create ICANN server from TCP listener: {e}");
        })?;

        tokio::spawn(
            icann_server
                .handle(handle.clone())
                .serve(router.into_make_service())
                .inspect_err(|e| error!("Nexus API ICANN DNS endpoint error: {e}")),
        );

        Ok((handle, local_addr))
    }

    async fn start_pubky_tls_server(
        ctx: &ApiContext,
        router: Router,
    ) -> Result<(ServerHandle, SocketAddr), DynError> {
        let pubky_socket = ctx.api_config.pubky_listen_socket;
        let pubky_listener = TcpListener::bind(pubky_socket)
            .inspect_err(|e| error!("Failed to bind to Pubky socket {pubky_socket:?}: {e}"))?;
        pubky_listener.set_nonblocking(true).inspect_err(|e| {
            error!("Failed to set Pubky listener {pubky_socket:?} non-blocking: {e}")
        })?;
        let pubky_local_addr = pubky_listener
            .local_addr()
            .inspect_err(|e| error!("Failed to get local address after binding: {e})"))?;
        let pubky_handle = ServerHandle::new();

        let tls_server = axum_server::from_tcp(pubky_listener).inspect_err(|e| {
            error!("Failed to create TLS server from TCP listener: {e}");
        })?;

        tokio::spawn(
            tls_server
                .acceptor(Self::create_pubky_tls_acceptor(&ctx.keypair))
                .handle(pubky_handle.clone())
                .serve(router.into_make_service())
                .inspect_err(|e| error!("Nexus API pubky TLS endpoint error: {e}")),
        );

        Ok((pubky_handle, pubky_local_addr))
    }

    /// Returns the public_key of this server
    pub fn public_key(&self) -> PublicKey {
        self.ctx.keypair.public_key()
    }

    /// Get the URL of the icann http server.
    pub fn icann_http_url(&self) -> String {
        format!("http://{}", self.icann_http_socket)
    }

    /// Get the URL of the pubky tls server with the Pubky DNS name.
    pub fn pubky_tls_dns_url(&self) -> String {
        format!("https://{}", self.public_key())
    }

    /// Get the URL of the pubky tls server with the Pubky IP address.
    pub fn pubky_tls_ip_url(&self) -> String {
        format!("https://{}", self.pubky_tls_socket)
    }

    fn create_pubky_tls_acceptor(keypair: &Keypair) -> RustlsAcceptor {
        let tls_config = Arc::new(keypair.to_rpk_rustls_server_config());
        let rustls_config = RustlsConfig::from_config(tls_config);
        RustlsAcceptor::new(rustls_config)
    }
}

impl Drop for NexusApi {
    fn drop(&mut self) {
        let grace_period = Duration::from_secs(30);

        info!("Starting graceful shutdown...");
        self.icann_http_handle.graceful_shutdown(Some(grace_period));
        self.pubky_tls_handle.graceful_shutdown(Some(grace_period));
        info!("Nexus API shut down gracefully");
    }
}

fn derive_key_publisher_context(
    ctx: &ApiContext,
    local_pubky_tls_port: u16,
) -> KeyRepublisherContext {
    KeyRepublisherContext {
        public_ip: ctx.api_config.public_ip,
        // Reference and expose the port of the local address used in the axum socket, which is where the axum server is listening.
        // This resolves ports :0 to an OS-chosen one.
        public_pubky_tls_port: local_pubky_tls_port,
        keypair: ctx.keypair.clone(),
        pkarr_client: ctx.pkarr_client.clone(),
    }
}
