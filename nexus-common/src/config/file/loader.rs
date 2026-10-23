use crate::types::DynError;
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::fs;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file {path:?}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse config file {path:?}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: DynError,
    },

    #[error("Failed to write default config file {path:?}: {source}")]
    WriteDefault {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

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
            .map_err(|source| ConfigError::Read {
                path: config_file_path.to_path_buf(),
                source,
            })?;

        // Convert TOML to struct with error handling
        let config = Self::try_from_str(&s).map_err(|source| ConfigError::Parse {
            path: config_file_path.to_path_buf(),
            source,
        })?;

        Ok(config)
    }

    /// Loads the struct from a TOML file, first writing `default_toml` to it if the file is
    /// missing. An existing file is never overwritten, so an invalid one is an error.
    async fn load_or_create(
        path: impl AsRef<Path> + Send,
        default_toml: &str,
    ) -> Result<T, DynError> {
        let config_file_path = path.as_ref();

        if !config_file_path.exists() {
            let write_default = async {
                if let Some(dir) = config_file_path.parent() {
                    fs::create_dir_all(dir).await?;
                }
                fs::write(config_file_path, default_toml).await
            };
            write_default
                .await
                .map_err(|source| ConfigError::WriteDefault {
                    path: config_file_path.to_path_buf(),
                    source,
                })?;
            println!(
                "Wrote the default config file {}",
                config_file_path.display()
            );
        }

        Self::load(config_file_path).await
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use serde::Deserialize;
    use std::error::Error;

    use super::{ConfigError, ConfigLoader};

    #[derive(Debug, Deserialize, PartialEq)]
    struct Sample {
        value: u32,
    }

    #[async_trait]
    impl ConfigLoader<Sample> for Sample {}

    /// A missing file is written from the default, along with its missing directory, then loaded.
    #[tokio::test]
    async fn test_load_or_create_writes_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("sample.toml");

        let sample = Sample::load_or_create(&path, "value = 1\n").await.unwrap();

        assert_eq!(sample, Sample { value: 1 });
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "value = 1\n");
    }

    /// An existing file is loaded as it is, never replaced by the default, even when invalid.
    #[tokio::test]
    async fn test_load_or_create_keeps_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.toml");

        std::fs::write(&path, "value = 2\n").unwrap();
        let sample = Sample::load_or_create(&path, "value = 1\n").await.unwrap();
        assert_eq!(sample, Sample { value: 2 });

        std::fs::write(&path, "value = \"two\"\n").unwrap();
        assert!(Sample::load_or_create(&path, "value = 1\n").await.is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "value = \"two\"\n");
    }
    /// A missing file returns a ConfigError::Read with the source preserved.
    #[tokio::test]
    async fn test_load_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does_not_exist.toml");

        let result = Sample::load(&path).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        let config_err = err
            .downcast_ref::<ConfigError>()
            .expect("Should be ConfigError");

        assert!(matches!(config_err, ConfigError::Read { .. }));
        assert!(config_err.source().is_some());
    }

    /// An invalid TOML file returns a ConfigError::Parse with the source preserved.
    #[tokio::test]
    async fn test_load_invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("invalid.toml");

        // Write malformed TOML
        std::fs::write(&path, "value = \"not_a_number\"\n").unwrap();

        let result = Sample::load(&path).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        let config_err = err
            .downcast_ref::<ConfigError>()
            .expect("Should be ConfigError");

        assert!(matches!(config_err, ConfigError::Parse { .. }));
        assert!(config_err.source().is_some());
    }
}
