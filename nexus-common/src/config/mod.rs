use serde::{Deserialize, Serialize};
use std::fmt::Debug;

pub const LOG_LEVEL: Level = Level::Debug;
pub const FILES_DIR: &str = "~/.pubky-nexus/static/files";
// All the tests run inside their own crate therefore the default directory does not apply
pub const FILES_DIR_TEST: &str = "./static/files";

mod api;
mod daemon;
pub mod file;
mod stack;
mod watcher;

pub use api::ApiConfig;
pub use daemon::DaemonConfig;
pub use stack::{default_stack, StackConfig};
pub use watcher::WatcherConfig;

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
