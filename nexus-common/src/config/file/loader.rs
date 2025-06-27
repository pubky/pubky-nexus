use crate::types::DynError;
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::path::Path;
use tokio::fs;

#[async_trait]
pub trait ConfigLoader<T>
where
    T: DeserializeOwned + Send + Sync + Debug,
{
    /// Parses the struct from a TOML string
    fn try_from_str(value: &str) -> Result<T, DynError> {
        let config_toml: T = toml::from_str(value)?;
        Ok(config_toml)
    }

    /// Loads the struct from a TOML file
    async fn load(path: impl AsRef<Path> + Send) -> Result<T, DynError> {
        let config_file_path = path.as_ref();

        // Read file with error handling
        let s = fs::read_to_string(config_file_path)
            .await
            .map_err(|e| format!("!Failed to read config file {config_file_path:?}: {e}"))?;

        // Convert TOML to struct with error handling
        let config = Self::try_from_str(&s)
            .map_err(|e| format!("Failed to parse config file {config_file_path:?}: {e}"))?;

        Ok(config)
    }
}
