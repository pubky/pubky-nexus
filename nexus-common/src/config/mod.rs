use serde::{Deserialize, Serialize};
use std::{fmt::Debug, path::PathBuf, sync::OnceLock};

pub const LOG_LEVEL: Level = Level::Info;

/// Path to the directory where static files are stored. To access this as a [PathBuf], use [get_files_dir_pathbuf].
pub const FILES_DIR: &str = "~/.pubky-nexus/static/files";
static FILES_DIR_PATHBUF: OnceLock<PathBuf> = OnceLock::new();
/// See [FILES_DIR]
pub fn get_files_dir_pathbuf() -> PathBuf {
    FILES_DIR_PATHBUF
        .get_or_init(|| {
            try_expand_home_dir(PathBuf::from(FILES_DIR))
                .expect("Hardcoded FILES_DIR should be a valid directory path")
        })
        .clone()
}

// All the tests run inside their own crate therefore the default directory does not apply
pub const FILES_DIR_TEST: &str = "./static/files";
static FILES_DIR_TEST_PATHBUF: OnceLock<PathBuf> = OnceLock::new();
pub fn get_files_dir_test_pathbuf() -> PathBuf {
    FILES_DIR_TEST_PATHBUF
        .get_or_init(|| {
            try_expand_home_dir(PathBuf::from(FILES_DIR_TEST))
                .expect("Hardcoded FILES_DIR_TEST should be a valid directory path")
        })
        .clone()
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

use crate::file::try_expand_home_dir;

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
