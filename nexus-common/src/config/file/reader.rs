use crate::types::DynError;
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use std::ffi::OsStr;
use std::fmt::Debug;
use std::path::{Component, Path, PathBuf};
use tracing::error;

use super::ConfigLoader;

/// Path to default nexusd config file. Defaults to ~/.pubky-nexus
pub const DEFAULT_HOME_DIR: &str = ".pubky-nexus";
const DEFAULT_CONFIG_TOML: &str = include_str!("../default.config.toml");
/// The sole configuration file name recognized by nexus
pub const CONFIG_FILE_NAME: &str = "config.toml";

/// Expands the data directory to the home directory if it starts with "~"
/// Return the full path to the data directory
pub fn expand_home_dir(path: PathBuf) -> PathBuf {
    if let Some(first) = path.components().next() {
        if first == Component::Normal(OsStr::new("~")) {
            if let Some(home) = dirs::home_dir() {
                // drop the "~" prefix and re-join
                let without_tilde = path.iter().skip(1).collect::<PathBuf>();
                return home.join(without_tilde);
            }
        }
    }
    path
}

#[async_trait]
pub trait ConfigReader<T>: ConfigLoader<T>
where
    T: DeserializeOwned + Send + Sync + Debug,
{
    /// Returns the config file path in this directory
    fn get_config_file_path(expanded_path: &Path) -> PathBuf {
        expanded_path.join(CONFIG_FILE_NAME)
    }

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
        std::fs::write(config_file_path, DEFAULT_CONFIG_TOML)?;
        Ok(())
    }

    /// Reads the config file from the data directory
    /// Creates a default config file if it doesn't exist
    async fn read_config_file(expanded_path: PathBuf) -> Result<T, DynError> {
        let config_file_path = Self::get_config_file_path(&expanded_path);

        if !config_file_path.exists() {
            Self::write_default_config_file(&config_file_path)?;
        }
        println!(
            "nexusd reading the '{CONFIG_FILE_NAME}' file from '{}'",
            expanded_path.display()
        );

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
