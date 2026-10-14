use std::time::Duration;

/// Name of the watcher config file
pub const WATCHER_CONFIG_FILE_NAME: &str = "watcher-config.toml";
/// Written to [WATCHER_CONFIG_FILE_NAME] by [NexusWatcher::start_from_path](super::NexusWatcher::start_from_path)
/// when the file is missing
pub(crate) const DEFAULT_WATCHER_CONFIG_TOML: &str =
    include_str!("../../default.watcher-config.toml");
/// How long a processor run may take before it is logged as slow
// TODO(#1014): read it, and the run budget, from the config file
pub const SLOW_RUN_THRESHOLD: Duration = Duration::from_secs(3_600);
/// How long a run of the external loop may spend on one homeserver before it stops, keeping
/// the cursors it reached, so one homeserver cannot hold up the others. A homeserver should
/// not need more than this, so a run that spends it backs the homeserver off like a failure,
/// doubling up to `max_backoff_secs`: one whose pass keeps needing more gets one budget per
/// `max_backoff_secs` (1 h by default).
pub const KEY_BASED_RUN_BUDGET: Duration = Duration::from_secs(8 * 60);
/// How long an external homeserver's run may take before it is logged as slow: two minutes
/// over [KEY_BASED_RUN_BUDGET], which a run only overshoots by the fetch or event in progress
pub const KEY_BASED_SLOW_RUN_THRESHOLD: Duration = Duration::from_secs(10 * 60);
