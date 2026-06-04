use crate::dispatcher::EventDispatcher;
use crate::service::NexusWatcher;
use nexus_common::db::{DatabaseConfig, PubkyConnector};
use nexus_common::plugin::{NexusPlugin, PluginContext};
use nexus_common::types::DynError;
use nexus_common::utils::create_shutdown_rx;
use nexus_common::WatcherConfig;
use nexus_common::{Level, StackConfig, StackManager};
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch::Receiver;

#[derive(Default)]
pub struct NexusWatcherBuilder {
    pub config: WatcherConfig,
    pub plugins: Vec<Arc<dyn NexusPlugin>>,
}

impl std::fmt::Debug for NexusWatcherBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NexusWatcherBuilder")
            .field("config", &self.config)
            .field(
                "plugins",
                &self
                    .plugins
                    .iter()
                    .map(|p| p.manifest().name)
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl NexusWatcherBuilder {
    /// Creates a `NexusWatcherBuilder` instance with the given configuration and stack settings.
    pub fn with_stack(mut config: WatcherConfig, stack: &StackConfig) -> Self {
        config.stack = stack.clone();
        Self {
            config,
            plugins: vec![],
        }
    }

    /// Register domain plugins. Each plugin's namespace is claimed in the dispatcher.
    pub fn with_plugins(mut self, plugins: Vec<Arc<dyn NexusPlugin>>) -> Self {
        self.plugins = plugins;
        self
    }

    /// Configures the logging level for the service, determining verbosity and log output
    pub fn log_level(&mut self, log_level: Level) -> &mut Self {
        self.config.stack.log_level = log_level;
        self
    }

    pub fn testnet(&mut self, testnet: bool) -> &mut Self {
        self.config.testnet = testnet;
        self
    }

    pub fn homeserver(&mut self, homeserver: PubkyId) -> &mut Self {
        self.config.homeserver = homeserver;
        self
    }

    /// Sets the directory for storing static files on the server
    pub fn files_path(&mut self, files_path: PathBuf) -> &mut Self {
        self.config.stack.files_path = files_path;
        self
    }

    /// Sets the OpenTelemetry endpoint for tracing and monitoring
    pub fn otlp_endpoint(&mut self, otlp_endpoint: Option<String>) -> &mut Self {
        self.config.stack.otlp.endpoint = otlp_endpoint;
        self
    }

    /// Sets the database configuration, including graph database and Redis settings
    pub fn db(&mut self, db: DatabaseConfig) -> &mut Self {
        self.config.stack.db = db;
        self
    }

    /// Opens DB connections and initialises tracing layer (if provided in config)
    pub async fn init_stack(&self) -> Result<(), DynError> {
        StackManager::setup(&self.config.stack).await?;
        let testnet_host = if self.config.testnet {
            Some(self.config.testnet_host.as_str())
        } else {
            None
        };
        let _ = PubkyConnector::initialise(testnet_host).await;
        Ok(())
    }

    /// Initializes the watcher integration test stack
    pub async fn init_test_stack(&self) -> Result<(), DynError> {
        StackManager::setup(&self.config.stack).await?;
        Ok(())
    }

    /// Initializes the service stack and starts the NexusWatcher event loop.
    ///
    /// If plugins are registered, their schemas are set up before the loop starts.
    ///
    /// ### Arguments
    ///
    /// - `shutdown_rx`: optional shutdown signal. If none is provided, a default one will be created, listening for Ctrl-C.
    pub async fn start(self, shutdown_rx: Option<Receiver<bool>>) -> Result<(), DynError> {
        let shutdown_rx = shutdown_rx.unwrap_or_else(create_shutdown_rx);

        self.init_stack().await?;

        // Setup Neo4j schema for each registered plugin (idempotent).
        // Failure is intentionally fatal: a plugin whose schema is missing
        // would silently index incomplete data, which is harder to detect
        // and recover from than a clean startup failure.
        for plugin in &self.plugins {
            plugin
                .setup_schema(&PluginContext::for_plugin(plugin.as_ref()))
                .await?;
        }

        let dispatcher = if self.plugins.is_empty() {
            None
        } else {
            Some(Arc::new(EventDispatcher::new(self.plugins)))
        };

        NexusWatcher::start(shutdown_rx, self.config, dispatcher).await
    }
}
