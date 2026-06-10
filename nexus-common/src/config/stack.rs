use crate::{db::DatabaseConfig, get_files_dir_pathbuf};
use pubky_app_specs::PubkyId;
use serde::{Deserialize, Deserializer, Serialize};
use std::{fmt::Debug, path::PathBuf};

use super::{file::validate_and_expand_path, Level, LOG_LEVEL};

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

    /// External HS PKs which are forbidden from being indexed
    #[serde(default)]
    pub external_hs_pk_blacklist: Vec<PubkyId>,
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
            external_hs_pk_blacklist: Vec::new(),
        }
    }
}
