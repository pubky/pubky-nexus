use super::MigrationManager;
use async_trait::async_trait;
use nexus_common::file::ConfigLoader;
use nexus_common::file::ConfigReader;
use nexus_common::types::DynError;
use nexus_common::StackConfig;
use nexus_common::StackManager;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt::Debug;

/// Path to default migration config file. Defaults to ~/.pubky-nexus/migrations
pub const CONFIG_FILE: &str = ".pubky-nexus/migrations";
pub const TRACER_NAME: &str = "nexus.migration";

// Nexus API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationConfig {
    pub name: String,
    pub backfill_ready: Vec<String>,
    // TODO: Choose a right name
    pub stack: StackConfig,
}

#[derive(Debug)]
pub struct MigrationBuilder(pub(crate) MigrationConfig);

impl MigrationBuilder {
    pub async fn default() -> MigrationBuilder {
        let config_file = dirs::home_dir().unwrap_or_default().join(CONFIG_FILE);
        let config: MigrationConfig =
            match MigrationConfig::read_config_file(config_file, true).await {
                Ok(c) => c,
                Err(e) => panic!("Error with migration config file, {:?}", e),
            };
        MigrationBuilder(config)
    }

    pub async fn init_stack(&self) -> Result<MigrationManager, DynError> {
        // Open ddbb connections and init tracing layer
        StackManager::setup(&self.0.name, &self.0.stack).await;
        Ok(MigrationManager::default())
    }

    pub fn migrations_backfill_ready(self) -> Vec<String> {
        self.0.backfill_ready
    }
}

#[async_trait]
impl<T> ConfigLoader<T> for MigrationConfig where T: DeserializeOwned + Send + Sync + Debug {}
