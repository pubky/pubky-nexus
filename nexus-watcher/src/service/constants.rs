use nexus_common::types::DynError;

/// Name of the watcher config file
pub const WATCHER_CONFIG_FILE_NAME: &str = "watcher-config.toml";
// TODO: Maybe that ones should be configurable? .toml file?
/// Max homeservers processed concurrently within a cycle
pub const MAX_CONCURRENT: usize = 3;
///  Per-homeserver hard timeout (seconds)
pub const PROCESSING_TIMEOUT_SECS: u64 = 120;

/// Result of a homeserver event processing
pub enum ProcessResult {
    Success(String),
    Error(DynError),
    Panic(tokio::task::JoinError),
}