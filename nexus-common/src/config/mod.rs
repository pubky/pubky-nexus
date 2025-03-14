mod loader;
pub use loader::ConfigLoader;

use crate::db::DatabaseConfig;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, path::PathBuf};

pub const LOG_LEVEL: Level = Level::Debug;
pub const FILES_DIR: &str = "./nexus-api/static/files";

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// Designates very low priority, often extremely verbose, information.
    Trace,
    /// Designates lower priority information.
    Debug,
    /// Designates useful information.
    Info,
    /// Designates hazardous situations.
    Warn,
    /// Designates very serious errors.
    Error,
}

impl Level {
    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Trace => "trace",
            Level::Debug => "debug",
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Config {
    pub log_level: Level,
    pub files_path: PathBuf,
    pub otlp_endpoint: Option<String>,
    pub db: DatabaseConfig,
}

/// Utility function
pub fn default_stack() -> Config {
    Config::default()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            log_level: LOG_LEVEL,
            files_path: PathBuf::from(FILES_DIR),
            otlp_endpoint: None,
            db: DatabaseConfig::default(),
        }
    }
}
