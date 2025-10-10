/// Name of the watcher config file
pub const WATCHER_CONFIG_FILE_NAME: &str = "watcher-config.toml";
///  Per-homeserver hard timeout (seconds)
// TODO: Set timeout maybe from the config file
pub const PROCESSING_TIMEOUT_SECS: u64 = 3_600;
/// Maximum number of homeservers from which events are fetched and processed
pub const MAX_HOMESERVERS_PER_RUN: usize = 50;
