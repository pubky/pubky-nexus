use async_trait::async_trait;
use nexus_common::file::ConfigLoader;
use nexus_common::types::DynError;
use nexus_common::StackConfig;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt::Debug;
use std::path::PathBuf;

/// Migrations config subdirectory, resolved under the daemon config dir
/// (defaults to ~/.pubky-nexus/migrations)
pub const MIGRATIONS_CONFIG_DIR: &str = "migrations";
pub const MIGRATIONS_CONFIG_FILE_NAME: &str = "config.toml";
const DEFAULT_CONFIG_TOML: &str = include_str!("default.config.toml");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationConfig {
    pub backfill_ready: Vec<String>,
    pub stack: StackConfig,
}

#[derive(Debug)]
pub struct MigrationBuilder(pub(crate) MigrationConfig);

impl MigrationBuilder {
    pub async fn new(config_dir: PathBuf) -> Result<MigrationBuilder, DynError> {
        let config_file_path = config_dir
            .join(MIGRATIONS_CONFIG_DIR)
            .join(MIGRATIONS_CONFIG_FILE_NAME);
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
                    "Validating existence of '{}' and creating it if missing before copying '{MIGRATIONS_CONFIG_FILE_NAME}' file…",
                    parent.display()
                );
                std::fs::create_dir_all(parent)?;
            }
            // Create the file
            std::fs::write(config_file_path, DEFAULT_CONFIG_TOML)?;
        }
        Ok(())
    }

    pub fn stack(&self) -> &StackConfig {
        &self.0.stack
    }

    pub fn migrations_backfill_ready(self) -> Vec<String> {
        self.0.backfill_ready
    }
}

#[async_trait]
impl<T> ConfigLoader<T> for MigrationConfig where T: DeserializeOwned + Send + Sync + Debug {}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn new_resolves_config_under_supplied_config_dir() {
        let config_dir = tempfile::tempdir().expect("tempdir should be created");

        let builder = MigrationBuilder::new(config_dir.path().to_path_buf())
            .await
            .expect("builder should load the config");

        let expected_path = config_dir
            .path()
            .join(MIGRATIONS_CONFIG_DIR)
            .join(MIGRATIONS_CONFIG_FILE_NAME);
        assert!(
            expected_path.exists(),
            "default migration config should be written under the supplied config dir"
        );
        // The written default config has no backfill_ready entries.
        assert!(builder.0.backfill_ready.is_empty());
    }
}
