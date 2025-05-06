use crate::types::DynError;
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use tracing::error;

use super::ConfigLoader;

pub const DEFAULT_HOME_DIR: &str = ".pubky-nexus";
const TEMPLATE_CONFIG_FILE: &str = "nexusd/conf_template.toml";
const MIGRATION_TEMPLATE_CONFIG_FILE: &str = "nexusd/src/migrations/conf_template.toml";
const CONFIG_FILE_NAME: &str = "config.toml";

/// Expands the data directory to the home directory if it starts with "~"
/// Return the full path to the data directory.
pub fn expand_home_dir(path: PathBuf) -> PathBuf {
    let path = match path.to_str() {
        Some(path) => path,
        None => {
            // Path not valid utf-8 so we can't expand it.
            return path;
        }
    };

    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            let without_home = path.strip_prefix("~/").expect("Invalid ~ prefix");
            let joined = home.join(without_home);
            return joined;
        }
    }
    PathBuf::from(path)
}

#[async_trait]
pub trait ConfigReader<T>: ConfigLoader<T>
where
    T: DeserializeOwned + Send + Sync + Debug,
{
    /// Returns the config file path in this directory.
    fn get_config_file_path(expanded_path: &Path) -> PathBuf {
        expanded_path.join(CONFIG_FILE_NAME)
    }

    fn write_default_config_file(
        config_file_path: &PathBuf,
        migration_file: bool,
    ) -> std::io::Result<()> {
        // Copy the default path
        if let Some(parent) = config_file_path.parent() {
            println!(
                "creating a new directory at {:?} and copying 'conf.toml' into it",
                parent
            );
            std::fs::create_dir_all(parent)?;
        }
        // Define the template config file path
        let file_path = if migration_file {
            MIGRATION_TEMPLATE_CONFIG_FILE
        } else {
            TEMPLATE_CONFIG_FILE
        };
        // Copy the file
        std::fs::copy(PathBuf::from(file_path), config_file_path)?;
        Ok(())
    }

    /// Reads the config file from the data directory.
    /// Creates a default config file if it doesn't exist.
    async fn read_config_file(expanded_path: PathBuf, migration_file: bool) -> Result<T, DynError> {
        println!("nexusd reading the conf.toml file from {:?}", expanded_path);
        let config_file_path = Self::get_config_file_path(&expanded_path);
        if !config_file_path.exists() {
            Self::write_default_config_file(&config_file_path, migration_file)?;
        }
        let config = <Self as ConfigLoader<T>>::load(config_file_path)
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

// ——————————————————————————————————————————————————————————————
// Blanket impl so *any* `T` automatically gets a `ConfigReader<T>`
// ——————————————————————————————————————————————————————————————
#[async_trait]
impl<T> ConfigReader<T> for T where T: ConfigLoader<T> + DeserializeOwned + Send + Sync + Debug {}
