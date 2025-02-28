use std::{net::SocketAddr, path::PathBuf};

use tokio::net::TcpListener;
use tracing::{debug, info, Level};

use crate::{common::{DatabaseConfig, FILES_DIR, LOG_LEVEL}, routes, StackManager};

pub const NAME: &str = "nexus.api";
pub const DEFAULT_HOST: [u8; 4] = [127, 0, 0, 1];
pub const DEFAULT_PORT: u16 = 8080;

// Nexus API configuration
#[derive(Debug, Clone)]
pub struct Config {
    pub name: String,
    pub public_addr: SocketAddr,
    pub log_level: Level,
    pub files_path: PathBuf,
    pub otlp_endpoint: Option<String>,
    pub db: DatabaseConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: String::from(NAME),
            public_addr: SocketAddr::from((DEFAULT_HOST, DEFAULT_PORT)),
            log_level: LOG_LEVEL,
            files_path: PathBuf::from(FILES_DIR),
            otlp_endpoint: None,
            db: DatabaseConfig::default(),
        }
    }
}

#[derive(Debug, Default)]
//pub struct NexusApiBuilder(Config);
pub struct NexusApiBuilder(pub(crate) Config);

impl NexusApiBuilder {
    /// Set the Homeserver's keypair
    pub fn name(&mut self, name: String) -> &mut Self {
        self.0.name = name;

        self
    }

    pub fn public_addr(&mut self, addr: SocketAddr) -> &mut Self {
        self.0.public_addr = addr;

        self
    }

    pub fn files_path(&mut self, files_path: PathBuf) -> &mut Self {
        self.0.files_path = files_path;

        self
    }

    pub fn otlp_endpoint(&mut self, otlp_endpoint: Option<String>) -> &mut Self {
        self.0.otlp_endpoint = otlp_endpoint;

        self
    }

    pub fn db(&mut self, db: DatabaseConfig) -> &mut Self {
        self.0.db = db;

        self
    }

    // TODO: Maybe create in common the initialisation of the stack
    pub async fn init_stack(&self) {
        // Open ddbb connections and init tracing layer
        StackManager::setup(
            &self.0.name,
            &self.0.otlp_endpoint,
            self.0.log_level,
            &self.0.db,
        )
        .await;
    }

    pub async fn run(self) {
        self.init_stack().await;
        NexusApi::run(self.0).await
    }

    pub async fn run_test(self, listener: TcpListener) {
        NexusApi::run_test(self.0, listener).await
    }
}

pub struct NexusApi {}

impl NexusApi {
    pub fn builder() -> NexusApiBuilder {
        NexusApiBuilder::default()
    }

    pub fn run_with_config_file() -> NexusApiBuilder {
        NexusApiBuilder::default()
    }

    pub async fn run(config: Config) {
        // Create all the routes of the API
        let app = routes::routes(config.files_path.clone());
        debug!(?config, "Running NexusAPI with ");

        // Start server
        let listener = TcpListener::bind(config.public_addr).await.unwrap();
        info!("Listening on {:?}\n", listener.local_addr().unwrap());

        axum::serve(listener, app.into_make_service())
            .await
            .unwrap();
    }

    // TODO: From now this is a patch. Find out how to do it better. Mainly for tests
    pub async fn run_test(config: Config, listener: TcpListener) {
        // Create all the routes of the API
        let app = routes::routes(config.files_path.clone());
        debug!(?config, "Running NexusAPI with ");

        // Start server
        info!("Listening on {:?}\n", listener.local_addr().unwrap());

        axum::serve(listener, app.into_make_service())
            .await
            .unwrap();
    }
}
