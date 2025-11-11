use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, path::PathBuf};
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
    fn get_config_file_path(expanded_path: PathBuf) -> PathBuf {
        expanded_path.join(CONFIG_FILE_NAME)
    }

    /// Writes the default [DaemonConfig] config file into the specified path
    fn write_default_config_file(config_file_path: PathBuf) -> std::io::Result<()> {
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
        let config_file_path = Self::get_config_file_path(expanded_path);

        if !config_file_path.exists() {
            Self::write_default_config_file(config_file_path.clone())?;
        }

        println!("nexusd loading config file {}", config_file_path.display());
        Self::load(&config_file_path).await.inspect_err(|e| {
            error!("Failed to load config file: {e}");
        })
    }
}

#[async_trait]
impl ConfigLoader<DaemonConfig> for DaemonConfig {}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, path::PathBuf, str::FromStr};

    use pubky_app_specs::PubkyId;

    use crate::{file::validate_and_expand_path, DaemonConfig, Level};

    #[tokio_shared_rt::test(shared)]
    async fn test_toml_parsing() {
        let c: DaemonConfig = DaemonConfig::read_or_create_config_file(
            tempfile::TempDir::new().unwrap().path().to_path_buf(),
        )
        .await
        .unwrap();

        assert_eq!(c.api.name, "nexusd.api");
        assert_eq!(c.api.public_addr, SocketAddr::from(([127, 0, 0, 1], 8080)));

        assert_eq!(c.watcher.name, "nexusd.watcher");
        assert!(!c.watcher.testnet);
        assert!(c.watcher.decentralization);
        assert_eq!(
            c.watcher.homeserver,
            PubkyId::try_from("8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty").unwrap()
        );
        assert_eq!(c.watcher.events_limit, 50);
        assert_eq!(c.watcher.watcher_sleep, 5_000);
        assert_eq!(
            c.watcher.moderation_id,
            PubkyId::try_from("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap()
        );
        assert_eq!(
            c.watcher.moderated_tags,
            vec![
                "hatespeech",
                "harassement",
                "terrorism",
                "violence",
                "illegal_activities",
                "il_adult_nu_sex_act",
            ]
        );

        assert_eq!(c.stack.log_level, Level::Info);
        assert_eq!(
            c.stack.files_path,
            validate_and_expand_path(PathBuf::from_str("~/.pubky-nexus/static/files").unwrap())
                .unwrap()
        );
        assert_eq!(c.stack.db.redis, "redis://127.0.0.1:6379");
        assert_eq!(c.stack.db.neo4j.uri, "bolt://localhost:7687");
    }
}
