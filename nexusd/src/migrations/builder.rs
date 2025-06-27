use super::MigrationManager;
use async_trait::async_trait;
use nexus_common::file::ConfigLoader;
use nexus_common::file::ConfigReader;
use nexus_common::file::CONFIG_FILE_NAME;
use nexus_common::types::DynError;
use nexus_common::StackConfig;
use nexus_common::StackManager;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt::Debug;
use std::path::PathBuf;

/// Path to default migration config file. Defaults to ~/.pubky-nexus/migrations
pub const CONFIG_FILE: &str = ".pubky-nexus/migrations";
pub const TRACER_NAME: &str = "nexus.migration";
const DEFAULT_CONFIG_TOML: &str = include_str!("default.config.toml");

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
    pub async fn default() -> Result<MigrationBuilder, DynError> {
        let config_folder = dirs::home_dir().unwrap_or_default().join(CONFIG_FILE);
        let config_file_path = MigrationConfig::get_config_file_path(&config_folder);
        Self::check_if_file_exists(&config_file_path)?;
        let config: MigrationConfig = match MigrationConfig::load(config_file_path).await {
            Ok(c) => c,
            Err(e) => panic!("Error with migration config file, {e:?}"),
        };
        Ok(MigrationBuilder(config))
    }

    fn check_if_file_exists(config_file_path: &PathBuf) -> std::io::Result<()> {
        if !config_file_path.exists() {
            // Make sure before write the file, the directory path exists
            if let Some(parent) = config_file_path.parent() {
                println!(
                    "Validating existence of '{}' and creating it if missing before copying '{CONFIG_FILE_NAME}' file…",
                    parent.display()
                );
                std::fs::create_dir_all(parent)?;
            }
            // Create the file
            std::fs::write(config_file_path, DEFAULT_CONFIG_TOML)?;
        }
        Ok(())
    }

    pub async fn init_stack(&self) -> Result<MigrationManager, DynError> {
        // Open ddbb connections and init tracing layer
        StackManager::setup(&self.0.name, &self.0.stack).await?;
        Ok(MigrationManager::default())
    }

    pub fn migrations_backfill_ready(self) -> Vec<String> {
        self.0.backfill_ready
    }
}

#[async_trait]
impl<T> ConfigLoader<T> for MigrationConfig where T: DeserializeOwned + Send + Sync + Debug {}
