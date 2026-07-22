use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{fmt::Debug, path::PathBuf};
use tracing::error;

use crate::{file::CONFIG_FILE_NAME, types::DynError};

use super::{file::ConfigLoader, ApiConfig, JobConfig, StackConfig, WatcherConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub watcher: WatcherConfig,
    pub stack: StackConfig,
    /// Scheduling config per cron job, keyed by job name (`[jobs.<name>]`).
    #[serde(default)]
    pub jobs: HashMap<String, JobConfig>,
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

    use crate::config::file::{reader::DEFAULT_CONFIG_TOML, ConfigLoader};
    use crate::{
        config::watcher::DEFAULT_MODERATION_ID, file::validate_and_expand_path, DaemonConfig, Level,
    };

    #[tokio_shared_rt::test(shared)]
    async fn test_toml_parsing() {
        let c: DaemonConfig = DaemonConfig::read_or_create_config_file(
            tempfile::TempDir::new().unwrap().path().to_path_buf(),
        )
        .await
        .unwrap();

        assert_eq!(c.api.public_addr, SocketAddr::from(([127, 0, 0, 1], 8080)));

        assert!(!c.stack.net.testnet);
        assert_eq!(c.stack.net.testnet_host, "localhost");
        assert_eq!(
            c.watcher.homeserver,
            PubkyId::try_from("8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty").unwrap()
        );
        assert_eq!(c.watcher.events_limit, 50);
        assert_eq!(c.watcher.key_based_events_limit, 50);
        assert_eq!(c.watcher.primary_hs_monitoring_interval_ms, 5_000);
        assert_eq!(c.watcher.external_hs_monitoring_interval_ms, 5_000);
        assert_eq!(c.watcher.hs_resolver_interval_ms, 10_000);
        assert_eq!(c.watcher.retry_processor_interval_ms, 10_000);
        assert_eq!(
            c.watcher.moderation_id,
            PubkyId::try_from(DEFAULT_MODERATION_ID).unwrap()
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
        assert!(c.stack.net.external_hs_pk_blacklist.is_empty());

        assert_eq!(c.stack.log_level, Level::Info);
        assert_eq!(
            c.stack.files_path,
            validate_and_expand_path(PathBuf::from_str("~/.pubky-nexus/static/files").unwrap())
                .unwrap()
        );
        assert_eq!(c.stack.otlp.name, "nexusd");
        assert!(c.stack.otlp.endpoint.is_none());
        assert_eq!(c.stack.db.redis, "redis://127.0.0.1:6379");
        assert_eq!(c.stack.db.neo4j.uri, "bolt://localhost:7687");

        // No jobs are registered/configured by default.
        assert!(c.jobs.is_empty());
    }

    /// A `[jobs.<name>]` section parses into a keyed [`JobConfig`], with its cron
    /// stored verbatim.
    #[test]
    fn test_job_config_parsing() {
        let toml = format!("{DEFAULT_CONFIG_TOML}\n[jobs.example]\ncron = \"0 0 3 * * *\"\n");

        let c = DaemonConfig::try_from_str(&toml).expect("config with a job section should parse");

        assert_eq!(c.jobs["example"].cron.as_deref(), Some("0 0 3 * * *"));
    }

    /// An absent cron leaves the job unscheduled (the default).
    #[test]
    fn test_job_config_defaults_to_no_cron() {
        let toml = format!("{DEFAULT_CONFIG_TOML}\n[jobs.example]\n");

        let c = DaemonConfig::try_from_str(&toml)
            .expect("config with an empty job section should parse");

        assert!(c.jobs["example"].cron.is_none());
    }

    /// A populated `external_hs_pk_blacklist` is parsed into the expected
    /// `Vec<PubkyId>`, preserving the order and number of entries.
    #[test]
    fn test_external_hs_pk_blacklist_parsing() {
        let hs1 = "8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty";
        let hs2 = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";

        let toml = DEFAULT_CONFIG_TOML.replace(
            "external_hs_pk_blacklist = []",
            &format!(r#"external_hs_pk_blacklist = ["{hs1}", "{hs2}"]"#),
        );

        let c = DaemonConfig::try_from_str(&toml)
            .expect("config with a populated blacklist should parse");

        assert_eq!(
            c.stack.net.external_hs_pk_blacklist,
            vec![
                PubkyId::try_from(hs1).unwrap(),
                PubkyId::try_from(hs2).unwrap(),
            ],
            "blacklist entries should parse, in order, into PubkyId values"
        );
    }

    /// An invalid public key in the blacklist must fail config parsing, since
    /// `PubkyId` validates the z32 encoding on deserialization.
    #[test]
    fn test_external_hs_pk_blacklist_rejects_invalid_pk() {
        let toml = DEFAULT_CONFIG_TOML.replace(
            "external_hs_pk_blacklist = []",
            r#"external_hs_pk_blacklist = ["not-a-valid-pubky"]"#,
        );

        assert!(
            DaemonConfig::try_from_str(&toml).is_err(),
            "an invalid public key in the blacklist must fail config parsing"
        );
    }

    /// Legacy `watcher_sleep` / `hs_resolver_sleep` field names (renamed to
    /// `*_interval_ms` to clarify they are scheduling intervals, not post-run
    /// sleeps) must still parse via serde aliases.
    #[test]
    fn test_legacy_watcher_interval_aliases_still_parse() {
        let toml = DEFAULT_CONFIG_TOML
            .replace(
                "primary_hs_monitoring_interval_ms = 5000",
                "watcher_sleep = 7000",
            )
            .replace(
                "hs_resolver_interval_ms = 10000",
                "hs_resolver_sleep = 20000",
            );

        let c = DaemonConfig::try_from_str(&toml).expect(
            "legacy watcher_sleep / hs_resolver_sleep field names must still parse via aliases",
        );

        assert_eq!(c.watcher.primary_hs_monitoring_interval_ms, 7_000);
        assert_eq!(c.watcher.hs_resolver_interval_ms, 20_000);
    }

    #[test]
    fn test_periodic_watcher_intervals_reject_zero() {
        let zero_intervals = [
            (
                "primary_hs_monitoring_interval_ms = 5000",
                "primary_hs_monitoring_interval_ms = 0",
            ),
            (
                "external_hs_monitoring_interval_ms = 5000",
                "external_hs_monitoring_interval_ms = 0",
            ),
            (
                "hs_resolver_interval_ms = 10000",
                "hs_resolver_interval_ms = 0",
            ),
            (
                "retry_processor_interval_ms = 10000",
                "retry_processor_interval_ms = 0",
            ),
        ];

        for (configured_value, zero_value) in zero_intervals {
            let toml = DEFAULT_CONFIG_TOML.replace(configured_value, zero_value);
            assert!(
                DaemonConfig::try_from_str(&toml).is_err(),
                "{zero_value} must be rejected"
            );
        }
    }
}
