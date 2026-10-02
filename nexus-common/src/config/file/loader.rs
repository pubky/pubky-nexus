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
            write_default.await.map_err(|e| {
                format!("Failed to write default config file {config_file_path:?}: {e}")
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

    use super::ConfigLoader;

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
}
