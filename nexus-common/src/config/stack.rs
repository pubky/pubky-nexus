use crate::{db::DatabaseConfig, get_files_dir_pathbuf};
use serde::{Deserialize, Deserializer, Serialize};
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

impl MediaConfig {
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
}

impl Default for OtlpConfig {
    fn default() -> Self {
        Self {
            name: String::from("nexus"),
            endpoint: None,
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
    fn test_max_concurrency_defaults_when_absent() {
        let config: MediaConfig = toml::from_str("").expect("empty table must use the default");
        assert!(config.max_concurrency >= 4);
    }
}
