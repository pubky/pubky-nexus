/// Name of the watcher config file
pub const WATCHER_CONFIG_FILE_NAME: &str = "watcher-config.toml";
/// Written to [WATCHER_CONFIG_FILE_NAME] by [NexusWatcher::start_from_path](super::NexusWatcher::start_from_path)
/// when the file is missing
pub(crate) const DEFAULT_WATCHER_CONFIG_TOML: &str =
    include_str!("../../default.watcher-config.toml");
///  Per-homeserver hard timeout (seconds)
// TODO: Set timeout maybe from the config file
pub const PROCESSING_TIMEOUT_SECS: u64 = 3_600;
