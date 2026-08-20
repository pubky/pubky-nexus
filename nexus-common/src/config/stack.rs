use crate::{db::DatabaseConfig, get_files_dir_pathbuf};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::{fmt::Debug, path::PathBuf};

use super::net::NetConfig;
use super::{file::validate_and_expand_path, Level, LOG_LEVEL};

/// Media processing concurrency configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct MediaConfig {
    /// Maximum number of concurrent media subprocesses (ImageMagick/ffmpeg).
    /// Defaults to the number of available parallelism (CPU cores), minimum 4.
    #[serde(
        default = "MediaConfig::default_max_concurrency",
        deserialize_with = "deserialize_max_concurrency"
    )]
    pub max_concurrency: usize,
    /// Wall-clock deadline for a single media subprocess, after which it is killed and reaped.
    /// A backstop above ImageMagick's own time limit, for children that hang without burning CPU.
    #[serde(
        default = "MediaConfig::default_process_timeout_secs",
        deserialize_with = "deserialize_process_timeout_secs"
    )]
    pub process_timeout_secs: u64,
}

/// Rejects 0, which would otherwise shed every variant request forever.
fn deserialize_max_concurrency<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    let max_concurrency = usize::deserialize(deserializer)?;
    if max_concurrency == 0 {
        return Err(serde::de::Error::custom(
            "stack.media.max_concurrency must be greater than 0",
        ));
    }
    Ok(max_concurrency)
}

/// Rejects 0, which would kill every subprocess before it could start.
fn deserialize_process_timeout_secs<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let process_timeout_secs = u64::deserialize(deserializer)?;
    if process_timeout_secs == 0 {
        return Err(serde::de::Error::custom(
            "stack.media.process_timeout_secs must be greater than 0",
        ));
    }
    Ok(process_timeout_secs)
}

impl MediaConfig {
    /// Well above an honest 100 MB resize, which runs in seconds to tens of seconds.
    const DEFAULT_PROCESS_TIMEOUT_SECS: u64 = 180;

    fn default_process_timeout_secs() -> u64 {
        Self::DEFAULT_PROCESS_TIMEOUT_SECS
    }

    fn default_max_concurrency() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .max(4)
    }
}

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            max_concurrency: Self::default_max_concurrency(),
            process_timeout_secs: Self::default_process_timeout_secs(),
        }
    }
}

fn deserialize_and_expand<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    let path: PathBuf = Deserialize::deserialize(deserializer)?;
    validate_and_expand_path(path).map_err(serde::de::Error::custom)
}

/// OpenTelemetry configuration for tracing, logging, and metrics export
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct OtlpConfig {
    /// Service name used for tracing, logging, and metrics in OpenTelemetry
    pub name: String,
    /// OTLP endpoint. When set, enables export of traces, logs, and metrics
    pub endpoint: Option<String>,
    /// Extra resource attributes attached to all traces, metrics, and logs
    /// from this process (e.g. `host`, `env`) so shared collectors can
    /// distinguish sources.
    #[serde(default)]
    pub resource_attributes: HashMap<String, String>,
}

impl Default for OtlpConfig {
    fn default() -> Self {
        Self {
            name: String::from("nexus"),
            endpoint: None,
            resource_attributes: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StackConfig {
    pub log_level: Level,
    #[serde(deserialize_with = "deserialize_and_expand")]
    pub files_path: PathBuf,
    #[serde(default)]
    pub otlp: OtlpConfig,
    pub db: DatabaseConfig,
    #[serde(default)]
    pub net: NetConfig,
    #[serde(default)]
    pub media: MediaConfig,
}

/// Utility function
pub fn default_stack() -> StackConfig {
    StackConfig::default()
}

impl Default for StackConfig {
    fn default() -> Self {
        Self {
            log_level: LOG_LEVEL,
            files_path: get_files_dir_pathbuf(),
            otlp: OtlpConfig::default(),
            db: DatabaseConfig::default(),
            net: NetConfig::default(),
            media: MediaConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MediaConfig;

    #[test]
    fn test_max_concurrency_parsing() {
        let cases = [
            ("max_concurrency = 1", Some(1)),
            ("max_concurrency = 16", Some(16)),
            // 0 permits would shed every variant request forever.
            ("max_concurrency = 0", None),
            ("max_concurrency = -1", None),
        ];

        for (toml, expected) in cases {
            let parsed = toml::from_str::<MediaConfig>(toml)
                .ok()
                .map(|c| c.max_concurrency);
            assert_eq!(parsed, expected, "unexpected result for {toml:?}");
        }
    }

    #[test]
    fn test_process_timeout_parsing() {
        let cases = [
            ("process_timeout_secs = 1", Some(1)),
            ("process_timeout_secs = 300", Some(300)),
            // 0 would kill every subprocess before it could start.
            ("process_timeout_secs = 0", None),
            ("process_timeout_secs = -1", None),
        ];

        for (toml, expected) in cases {
            let parsed = toml::from_str::<MediaConfig>(toml)
                .ok()
                .map(|c| c.process_timeout_secs);
            assert_eq!(parsed, expected, "unexpected result for {toml:?}");
        }
    }

    #[test]
    fn test_max_concurrency_defaults_when_absent() {
        let config: MediaConfig = toml::from_str("").expect("empty table must use the default");
        assert!(config.max_concurrency >= 4);
        assert!(config.process_timeout_secs > 0);
    }
}
