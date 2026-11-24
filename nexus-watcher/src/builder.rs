use crate::service::NexusWatcher;
use nexus_common::db::{build_pubky, DatabaseConfig};
use nexus_common::types::DynError;
use nexus_common::utils::create_shutdown_rx;
use nexus_common::WatcherConfig;
use nexus_common::{Level, StackManager};
use pubky_social_specs::PubkyId;
use pubky_watcher::WatcherClient;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch::Receiver;

#[derive(Debug, Default)]
pub struct NexusWatcherBuilder(pub WatcherConfig);

impl NexusWatcherBuilder {
    /// Configures the logging level for the service, determining verbosity and log output
    pub fn log_level(&mut self, log_level: Level) -> &mut Self {
        self.0.stack.log_level = log_level;

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
        self.0.stack.otlp.endpoint = otlp_endpoint;

        self
    }

    /// Sets the database configuration, including graph database and Redis settings
    pub fn db(&mut self, db: DatabaseConfig) -> &mut Self {
        self.0.stack.db = db;

        self
    }

    /// Builds the watcher's Pubky client from the `[stack.net]` settings, reading blobs up to
    /// [`WatcherConfig::max_file_size`]. Fails when the SDK client cannot be built.
    pub fn build_client(&self) -> Result<Arc<WatcherClient>, DynError> {
        let pubky = build_pubky(&self.0.stack.net)?;
        Ok(Arc::new(
            WatcherClient::from_shared(pubky).with_max_file_size(self.0.max_file_size),
        ))
    }

    /// Starts the NexusWatcher event loop, with a Pubky client of its own
    /// ([`Self::build_client`]). The watcher fails to start when it cannot be built.
    ///
    /// Calls [`StackManager::setup`] to initialize the shared infrastructure (logging, metrics, databases).
    /// If the stack was already initialized (e.g. by another builder), verifies the config matches.
    ///
    /// ### Arguments
    ///
    /// - `shutdown_rx`: optional shutdown signal. If none is provided, a default one will be created, listening for Ctrl-C.
    pub async fn start(self, shutdown_rx: Option<Receiver<bool>>) -> Result<(), DynError> {
        self.setup().await?;
        let client = self.build_client()?;
        self.run(shutdown_rx, client).await
    }

    /// Starts the NexusWatcher event loop over `client`, which it shares for every homeserver
    /// call and PKDNS lookup.
    ///
    /// See [`Self::start`] for the stack setup and `shutdown_rx`.
    pub async fn start_with_client(
        self,
        shutdown_rx: Option<Receiver<bool>>,
        client: Arc<WatcherClient>,
    ) -> Result<(), DynError> {
        self.setup().await?;
        self.run(shutdown_rx, client).await
    }

    /// Initializes the shared infrastructure, and the feature switches the watcher owns.
    async fn setup(&self) -> Result<(), DynError> {
        StackManager::setup(&self.0.stack).await?;
        // The watcher owns the feature switches: it stores them for every
        // service and brings the shared sets in line with a feature toggled
        // since the last start, before it processes events.
        self.0.features.toggle().await?;
        Ok(())
    }

    async fn run(
        self,
        shutdown_rx: Option<Receiver<bool>>,
        client: Arc<WatcherClient>,
    ) -> Result<(), DynError> {
        let shutdown_rx = shutdown_rx.unwrap_or_else(create_shutdown_rx);
        NexusWatcher::start(shutdown_rx, self.0, client).await
    }
}
