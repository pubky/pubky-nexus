use std::{fmt::Debug, path::PathBuf};

use super::MigrationManager;
use async_trait::async_trait;
use nexus_common::StackManager;
use nexus_common::{Config as StackConfig, ConfigLoader};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub const CONFIG_FILE: &str = "./src/migrations/mconf_template.toml";
pub const TRACER_NAME: &str = "nexus.migration";

// Nexus API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub name: String,
    pub backfill_ready: Vec<String>,
    // TODO: Choose a right name
    pub stack: StackConfig,
}

#[async_trait]
impl<T> ConfigLoader<T> for Config where T: DeserializeOwned + Send + Sync + Debug {}

#[derive(Debug)]
pub struct MigrationBuilder(pub(crate) Config);

impl MigrationBuilder {
    pub async fn default() -> MigrationBuilder {
        let config_file: PathBuf = CONFIG_FILE.into();
        let config: Config = match Config::load(config_file).await {
            Ok(c) => c,
            Err(e) => panic!("Error with migration config file, {:?}", e),
        };
        MigrationBuilder(config)
    }

    pub async fn init_stack(&self) -> MigrationManager {
        // Open ddbb connections and init tracing layer
        StackManager::setup(&self.0.name, &self.0.stack).await;
        MigrationManager::default()
    }

    pub fn migrations_backfill_ready(self) -> Vec<String> {
        self.0.backfill_ready
    }
}
