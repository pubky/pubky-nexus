use crate::db::DatabaseConfig;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, path::PathBuf};

use super::{Level, FILES_DIR, LOG_LEVEL};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct StackConfig {
    pub log_level: Level,
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
            files_path: PathBuf::from(FILES_DIR),
            otlp_endpoint: None,
            db: DatabaseConfig::default(),
        }
    }
}
