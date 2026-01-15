use crate::db::{default_files_path, default_log_level, default_otlp_endpoint, DatabaseConfig};
use serde::{Deserialize, Deserializer, Serialize};
use std::{fmt::Debug, path::PathBuf};

use super::{file::validate_and_expand_path, Level};

fn deserialize_and_expand<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    let path: PathBuf = Deserialize::deserialize(deserializer)?;
    validate_and_expand_path(path).map_err(serde::de::Error::custom)
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct StackConfig {
    pub log_level: Level,
    #[serde(deserialize_with = "deserialize_and_expand")]
    pub files_path: PathBuf,
    pub otlp_endpoint: Option<String>,
    pub db: DatabaseConfig,
}

/// Utility function
pub fn default_stack() -> StackConfig {
    StackConfig::default()
}

impl Default for StackConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            files_path: default_files_path(),
            otlp_endpoint: default_otlp_endpoint(),
            db: DatabaseConfig::default(),
        }
    }
}
