use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Debug,
    path::{Path, PathBuf},
};
use tracing::error;

use crate::{file::CONFIG_FILE_NAME, types::DynError};

use super::{file::ConfigLoader, ApiConfig, StackConfig, WatcherConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub watcher: WatcherConfig,
    pub stack: StackConfig,
}

impl DaemonConfig {
    /// Returns the config file path in this directory
    fn get_config_file_path(expanded_path: &Path) -> PathBuf {
        expanded_path.join(CONFIG_FILE_NAME)
    }

    /// Writes the default [DaemonConfig] config file into the specified path
    fn write_default_config_file(config_file_path: &PathBuf) -> std::io::Result<()> {
        // Make sure before write the file, the directory path exists
        if let Some(parent) = config_file_path.parent() {
            println!(
                "Validating existence of '{}' and creating it if missing before copying '{CONFIG_FILE_NAME}' file…",
                parent.display()
            );
            std::fs::create_dir_all(parent)?;
        }
        // Create the file
        std::fs::write(config_file_path, super::file::reader::DEFAULT_CONFIG_TOML)?;
        Ok(())
    }

    /// Given a directory path, ensures the directory exists, writes a default
    /// [DaemonConfig] file if absent, then parses and returns the loaded config
    pub async fn read_or_create_config_file(
        expanded_path: PathBuf,
    ) -> Result<DaemonConfig, DynError> {
        let config_file_path = Self::get_config_file_path(&expanded_path);

        if !config_file_path.exists() {
            Self::write_default_config_file(&config_file_path)?;
        }
        println!(
            "nexusd reading the '{CONFIG_FILE_NAME}' file from '{}'",
            expanded_path.display()
        );

        let config = <Self as ConfigLoader<DaemonConfig>>::load(config_file_path)
            .await
            .map_err(|e| {
                error!(
                    "Failed to load config file {:?}: {}",
                    Self::get_config_file_path(&expanded_path),
                    e
                );
                e
            })?;
        Ok(config)
    }
}

#[async_trait]
impl ConfigLoader<DaemonConfig> for DaemonConfig {}
