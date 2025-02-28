use std::path::PathBuf;
use crate::common::{FILES_DIR, LOG_LEVEL};
use pubky_app_specs::PubkyId;
use tokio::time::{sleep, Duration };
use tracing::{debug, error, info, Level};
use crate::{common::DatabaseConfig, types::DynError, EventProcessor, StackManager};

pub const NAME: &str = "nexus.watcher";
pub const TESTNET: bool = false;
pub const HOMESERVER_PUBKY: &str = "8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo";
// Maximum number of events to fetch at once from a homeserver
pub const EVENTS_LIMIT: u32 = 1000;
// Sleep between checks to homeserver
pub const WATCHER_SLEEP: u64 = 5000;

// Nexus API configuration
#[derive(Debug, Clone)]
pub struct Config {
    pub name: String,
    pub log_level: Level,
    pub testnet: bool,
    pub homeserver: PubkyId,
    pub events_limit: u32,
    pub watcher_sleep: u64,
    pub files_path: PathBuf,
    pub otlp_endpoint: Option<String>,
    pub db: DatabaseConfig,
}

impl Default for Config {
    fn default() -> Self {
        // TODO: not secure, could panic but maybe makes sense because it is an initialisation
        let homeserver = PubkyId::try_from(HOMESERVER_PUBKY).unwrap();
        Self {
            name: String::from(NAME),
            log_level: LOG_LEVEL,
            testnet: TESTNET,
            homeserver,
            events_limit: EVENTS_LIMIT,
            watcher_sleep: WATCHER_SLEEP,
            files_path: PathBuf::from(FILES_DIR),
            otlp_endpoint: None,
            db: DatabaseConfig::default(),
        }
    }
}

#[derive(Debug, Default)]
//pub struct NexusApiBuilder(Config);
pub struct NexusWatcherBuilder(pub(crate) Config);

impl NexusWatcherBuilder {
    /// Set the Homeserver's keypair
    pub fn name(&mut self, name: String) -> &mut Self {
        self.0.name = name;

        self
    }

    pub fn log_level(&mut self, log_level: Level) -> &mut Self {
        self.0.log_level = log_level;

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

    pub fn run_with_config_file() -> NexusWatcherBuilder {
        // TODO: next step, still to decide .toml or .env
        NexusWatcherBuilder::default()
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
