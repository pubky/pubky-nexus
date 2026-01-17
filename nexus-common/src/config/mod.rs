use serde::{Deserialize, Serialize};
use std::{fmt::Debug, path::PathBuf, sync::OnceLock};

use crate::db::{default_files_path, default_log_level};

/// Returns the default log level from `default.config.toml`.
/// Provided for backward compatibility. Use `StackConfig::default()` when possible.
pub fn get_default_log_level() -> Level {
    default_log_level()
}

// All the tests run inside their own crate therefore the default directory does not apply
pub const FILES_DIR_TEST: &str = "./static/files";
static FILES_DIR_TEST_PATHBUF: OnceLock<PathBuf> = OnceLock::new();
pub fn get_files_dir_test_pathbuf() -> PathBuf {
    FILES_DIR_TEST_PATHBUF
        .get_or_init(|| {
            validate_and_expand_path(PathBuf::from(FILES_DIR_TEST))
                .expect("Hardcoded FILES_DIR_TEST should be a valid directory path")
        })
        .clone()
}

/// Returns the default files directory path from `default.config.toml`.
/// See [default_files_path] for the implementation.
pub fn get_files_dir_pathbuf() -> PathBuf {
    default_files_path()
}

mod api;
mod daemon;
pub mod file;
mod stack;
mod watcher;

pub use api::ApiConfig;
pub use daemon::DaemonConfig;
pub use stack::{default_stack, StackConfig};
pub use watcher::WatcherConfig;

use crate::file::validate_and_expand_path;

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
