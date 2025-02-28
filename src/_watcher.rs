use std::path::PathBuf;
use pubky_app_specs::PubkyId;
use tokio::time::{sleep, Duration };
use tracing::{debug, error, info, Level};
use crate::{types::DynError, EventProcessor, StackManager};
use crate::common::{DatabaseConfig, Config as StackConfig};

pub const NAME: &str = "nexus.watcher";
pub const TESTNET: bool = false;
pub const HOMESERVER_PUBKY: &str = "8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo";
// Maximum number of events to fetch at once from a homeserver
pub const EVENTS_LIMIT: u32 = 1000;
// Sleep between checks to homeserver
pub const WATCHER_SLEEP: u64 = 5000;

// Nexus Watcher configuration
#[derive(Debug, Clone)]
pub struct Config {
    // TODO: Choose a right name
    pub stack: StackConfig,
    pub testnet: bool,
    pub homeserver: PubkyId,
    pub events_limit: u32,
    pub watcher_sleep: u64,
}

impl Default for Config {
    fn default() -> Self {
        // TODO: not secure, could panic but maybe makes sense because it is an initialisation
        let homeserver = PubkyId::try_from(HOMESERVER_PUBKY).unwrap();
        Self {
            stack: StackConfig::default(String::from(NAME)),
            testnet: TESTNET,
            homeserver,
            events_limit: EVENTS_LIMIT,
            watcher_sleep: WATCHER_SLEEP,
        }
    }
}

#[derive(Debug, Default)]
//pub struct NexusApiBuilder(Config);
pub struct NexusWatcherBuilder(pub(crate) Config);

impl NexusWatcherBuilder {
    /// Set the Homeserver's keypair
    pub fn name(&mut self, name: String) -> &mut Self {
        self.0.stack.name = name;

        self
    }

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

    pub fn files_path(&mut self, files_path: PathBuf) -> &mut Self {
        self.0.stack.files_path = files_path;

        self
    }

    pub fn otlp_endpoint(&mut self, otlp_endpoint: Option<String>) -> &mut Self {
        self.0.stack.otlp_endpoint = otlp_endpoint;

        self
    }

    pub fn db(&mut self, db: DatabaseConfig) -> &mut Self {
        self.0.stack.db = db;

        self
    }

    // TODO: Maybe create in common the initialisation of the stack
    pub async fn init_stack(&self) {
        // Open ddbb connections and init tracing layer
        StackManager::setup(
            &self.0.stack
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
