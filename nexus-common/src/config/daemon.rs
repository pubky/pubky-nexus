use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{fmt::Debug, path::PathBuf};
use tracing::error;

use crate::{file::CONFIG_FILE_NAME, types::DynError};

use super::{
    file::ConfigLoader, ApiConfig, JobConfig, StackConfig, TrustRankConfig, WatcherConfig,
};

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
    /// Trust-rank computation parameters (`[trust_rank]`).
    #[serde(default)]
    pub trust_rank: TrustRankConfig,
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
        config::watcher::DEFAULT_MODERATION_ID, default_trust_report_dir,
        file::validate_and_expand_path, DaemonConfig, Level, DEFAULT_TRUST_ALPHA,
        DEFAULT_TRUST_MAX_ITERATIONS, DEFAULT_TRUST_REPORT_LIMIT, DEFAULT_TRUST_TOLERANCE,
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

        let trust_job = c
            .jobs
            .get("trust-recompute")
            .expect("[jobs.trust-recompute] should be present");
        assert!(trust_job.cron.is_none());
        assert!(c.trust_rank.seed.is_empty());
        assert_eq!(c.trust_rank.alpha, DEFAULT_TRUST_ALPHA);
        assert_eq!(c.trust_rank.max_iterations, DEFAULT_TRUST_MAX_ITERATIONS);
        assert_eq!(c.trust_rank.tolerance, DEFAULT_TRUST_TOLERANCE);
        assert!(!c.trust_rank.report_enabled);
        assert_eq!(c.trust_rank.report_dir, default_trust_report_dir());
        assert_eq!(c.trust_rank.report_limit, DEFAULT_TRUST_REPORT_LIMIT);
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

    /// A valid `[jobs.trust-recompute] cron` expression parses and is stored verbatim.
    #[test]
    fn test_trust_job_cron_parsing() {
        let toml =
            DEFAULT_CONFIG_TOML.replace(r#"#cron = "0 0 3 * * *""#, r#"cron = "0 0 3 * * *""#);

        let c = DaemonConfig::try_from_str(&toml).expect("config with a valid cron should parse");

        assert_eq!(
            c.jobs["trust-recompute"].cron.as_deref(),
            Some("0 0 3 * * *")
        );
    }

    /// `trust_rank.report_enabled` and `trust_rank.report_dir` parse, with `~` expanded.
    #[test]
    fn test_trust_report_config_parsing() {
        let toml = DEFAULT_CONFIG_TOML
            .replace("report_enabled = false", "report_enabled = true")
            .replace(
                r#"report_dir = "~/.pubky-nexus/trust-reports""#,
                r#"report_dir = "~/custom/reports""#,
            );

        let c = DaemonConfig::try_from_str(&toml)
            .expect("config with trust reports enabled should parse");

        assert!(c.trust_rank.report_enabled);
        assert_eq!(
            c.trust_rank.report_dir,
            validate_and_expand_path(PathBuf::from_str("~/custom/reports").unwrap()).unwrap()
        );
    }

    /// A populated `trust_rank.seed` list parses into `PubkyId` entries, in order.
    #[test]
    fn test_trust_seed_parsing() {
        let hs1 = "8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty";
        let hs2 = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";

        let toml =
            DEFAULT_CONFIG_TOML.replace("seed = []", &format!(r#"seed = ["{hs1}", "{hs2}"]"#));

        let c = DaemonConfig::try_from_str(&toml).expect("config with a trust seed should parse");

        assert_eq!(
            c.trust_rank.seed,
            vec![
                PubkyId::try_from(hs1).unwrap(),
                PubkyId::try_from(hs2).unwrap(),
            ]
        );
    }

    /// Duplicate `trust_rank.seed` ids are silently deduplicated, preserving
    /// first-occurrence order.
    #[test]
    fn test_trust_seed_dedupes_duplicates() {
        let hs1 = "8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty";
        let hs2 = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";

        let toml = DEFAULT_CONFIG_TOML.replace(
            "seed = []",
            &format!(r#"seed = ["{hs1}", "{hs2}", "{hs1}"]"#),
        );

        let c =
            DaemonConfig::try_from_str(&toml).expect("config with duplicated seeds should parse");

        assert_eq!(
            c.trust_rank.seed,
            vec![
                PubkyId::try_from(hs1).unwrap(),
                PubkyId::try_from(hs2).unwrap(),
            ],
            "duplicate seed ids should be removed, keeping first-occurrence order"
        );
    }

    /// `trust_rank.alpha` must be in `(0, 1]`; out-of-range values fail config parsing.
    #[test]
    fn test_trust_alpha_rejects_out_of_range() {
        for bad_alpha in ["0.0", "1.5", "-0.1"] {
            let toml = DEFAULT_CONFIG_TOML.replace("alpha = 0.35", &format!("alpha = {bad_alpha}"));
            assert!(
                DaemonConfig::try_from_str(&toml).is_err(),
                "alpha = {bad_alpha} should be rejected"
            );
        }
    }

    /// An unknown key under `[trust_rank]` (e.g. a typo'd `report_enable`) must
    /// fail config parsing rather than parse silently into no effect.
    #[test]
    fn test_trust_rank_rejects_unknown_key() {
        let toml = format!("{DEFAULT_CONFIG_TOML}\nreport_enable = true\n");
        assert!(
            DaemonConfig::try_from_str(&toml).is_err(),
            "an unknown [trust_rank] key must be rejected"
        );
    }

    /// `trust_rank.max_iterations` must be at least 1; zero fails config parsing.
    #[test]
    fn test_trust_max_iterations_rejects_zero() {
        let toml = DEFAULT_CONFIG_TOML.replace("max_iterations = 200", "max_iterations = 0");
        assert!(
            DaemonConfig::try_from_str(&toml).is_err(),
            "max_iterations = 0 should be rejected"
        );
    }

    /// `report_limit` must be ≥ 1; zero fails parsing.
    #[test]
    fn test_trust_report_limit_rejects_zero() {
        let toml = DEFAULT_CONFIG_TOML.replace("report_limit = 10000", "report_limit = 0");
        assert!(
            DaemonConfig::try_from_str(&toml).is_err(),
            "report_limit = 0 should be rejected"
        );
    }

    /// `trust_rank.tolerance` must be finite and non-negative; bad values fail config parsing.
    #[test]
    fn test_trust_tolerance_rejects_invalid() {
        for bad_tolerance in ["-0.1", "nan", "inf"] {
            let toml = DEFAULT_CONFIG_TOML.replace(
                "tolerance = 0.0000001",
                &format!("tolerance = {bad_tolerance}"),
            );
            assert!(
                DaemonConfig::try_from_str(&toml).is_err(),
                "tolerance = {bad_tolerance} should be rejected"
            );
        }
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
