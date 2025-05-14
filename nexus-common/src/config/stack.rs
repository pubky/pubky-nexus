use crate::db::DatabaseConfig;
use serde::{Deserialize, Deserializer, Serialize};
use std::{fmt::Debug, path::PathBuf};

use super::{file::expand_home_dir, Level, FILES_DIR, LOG_LEVEL};

/// Custom deserializer: take a String, expand `~`, clean up `.`/`..`, return PathBuf.
fn deserialize_and_expand<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    let path: PathBuf = Deserialize::deserialize(deserializer)?;
    Ok(expand_home_dir(path))
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
            log_level: LOG_LEVEL,
            files_path: expand_home_dir(PathBuf::from(FILES_DIR)),
            otlp_endpoint: None,
            db: DatabaseConfig::default(),
        }
    }
}
