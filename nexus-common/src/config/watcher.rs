use super::file::ConfigLoader;
use super::{default_stack, DaemonConfig, StackConfig};
use async_trait::async_trait;
use pubky_app_specs::PubkyId;
use serde::{de::Error, Deserialize, Deserializer, Serialize};
use std::fmt::Debug;

// Testnet homeserver key
pub const HOMESERVER_PUBKY: &str = "8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo";
/// Default for [WatcherConfig::events_limit]
pub const DEFAULT_EVENTS_LIMIT: u16 = 1_000;
/// Default for [WatcherConfig::key_based_events_limit]
pub const DEFAULT_KEY_BASED_EVENTS_LIMIT: u16 = 50;
/// Default for [WatcherConfig::monitored_homeservers_limit]
pub const DEFAULT_MONITORED_HOMESERVERS_LIMIT: usize = 50;
/// Default for [WatcherConfig::default_hs_monitoring_interval_ms]
pub const DEFAULT_DEFAULT_HS_MONITORING_INTERVAL_MS: u64 = 5_000;
/// Default for [WatcherConfig::external_hs_monitoring_interval_ms]
pub const DEFAULT_EXTERNAL_HS_MONITORING_INTERVAL_MS: u64 = 5_000;
/// Default for [WatcherConfig::hs_resolver_interval_ms]
pub const DEFAULT_HS_RESOLVER_INTERVAL_MS: u64 = 10_000;
/// Default for [WatcherConfig::hs_resolver_ttl]: 1 hour in milliseconds
pub const DEFAULT_HS_RESOLVER_TTL: u64 = 3_600_000;
/// Default for [WatcherConfig::initial_backoff_secs]
pub const DEFAULT_INITIAL_BACKOFF_SECS: u64 = 60;
/// Default for [WatcherConfig::max_backoff_secs]
pub const DEFAULT_MAX_BACKOFF_SECS: u64 = 3_600;

/// Extra-safety check: Upper bound for [WatcherConfig::events_limit]
pub const MAX_EVENTS_LIMIT: u16 = 1_000;
/// Extra-safety check: Upper bound for [WatcherConfig::key_based_events_limit]
pub const MAX_KEY_BASED_EVENTS_LIMIT: u16 = 100;
/// Default for [WatcherConfig::max_file_size] — 50 MiB
pub const DEFAULT_MAX_FILE_SIZE: u64 = 50 * 1024 * 1024;

// Retry configuration defaults
/// Default for [EventRetryConfig::max_retries]
pub const DEFAULT_MAX_RETRIES: u32 = 10;
/// Default for [EventRetryConfig::max_dependency_retries]
pub const DEFAULT_MAX_DEPENDENCY_RETRIES: u32 = 50;
/// Default for [EventRetryConfig::initial_backoff_secs] (transient errors)
pub const DEFAULT_INITIAL_TRANSIENT_BACKOFF_SECS: u64 = 10;
/// Default for [EventRetryConfig::max_backoff_secs] (transient errors)
pub const DEFAULT_MAX_TRANSIENT_BACKOFF_SECS: u64 = 3_600;
/// Default for [EventRetryConfig::initial_missing_dep_backoff_secs]
pub const DEFAULT_INITIAL_MISSING_DEP_BACKOFF_SECS: u64 = 60;
/// Default for [EventRetryConfig::max_missing_dep_backoff_secs]
pub const DEFAULT_MAX_MISSING_DEP_BACKOFF_SECS: u64 = 3_600;

// Default moderation service key (test user key, overridden by config.toml value)
pub const DEFAULT_MODERATION_ID: &str = "uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko";
// Moderation service key
pub const MODERATED_TAGS: [&str; 6] = [
    "hatespeech",
    "harassement",
    "terrorism",
    "violence",
    "illegal_activities",
    "il_adult_nu_sex_act",
];

/// Event retry limits and backoff
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct EventRetryConfig {
    /// Transient error retry limit before dead-letter
    pub max_retries: u32,
    /// Safety net for homeservers that disappear silently (no DEL events, content just gone)
    pub max_dependency_retries: u32,
    /// Base for exponential backoff on transient retries (seconds)
    pub initial_backoff_secs: u64,
    /// Backoff ceiling for transient retries (seconds)
    pub max_backoff_secs: u64,
    /// Base for MissingDependency polling backoff (seconds)
    pub initial_missing_dep_backoff_secs: u64,
    /// Backoff ceiling for MissingDependency (seconds)
    pub max_missing_dep_backoff_secs: u64,
}

impl Default for EventRetryConfig {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            max_dependency_retries: DEFAULT_MAX_DEPENDENCY_RETRIES,
            initial_backoff_secs: DEFAULT_INITIAL_TRANSIENT_BACKOFF_SECS,
            max_backoff_secs: DEFAULT_MAX_TRANSIENT_BACKOFF_SECS,
            initial_missing_dep_backoff_secs: DEFAULT_INITIAL_MISSING_DEP_BACKOFF_SECS,
            max_missing_dep_backoff_secs: DEFAULT_MAX_MISSING_DEP_BACKOFF_SECS,
        }
    }
}

/// Configuration settings for the Nexus Watcher service
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct WatcherConfig {
    /// Default, prioritized homeserver
    pub homeserver: PubkyId,

    /// Maximum number of events to fetch per run from the default homeserver.
    /// Must not exceed [MAX_EVENTS_LIMIT].
    #[serde(deserialize_with = "deserialize_events_limit")]
    pub events_limit: u16,

    /// Maximum events per user per run for key-based (non-default) homeservers.
    /// Must not exceed [MAX_KEY_BASED_EVENTS_LIMIT].
    #[serde(
        default = "default_key_based_events_limit",
        deserialize_with = "deserialize_key_based_events_limit"
    )]
    pub key_based_events_limit: u16,

    /// Maximum number of monitored homeservers
    pub monitored_homeservers_limit: usize,

    /// Scheduling interval (ms) at which the default-HS monitoring task is triggered.
    /// The alias `watcher_sleep` is kept for backward compatibility.
    #[serde(alias = "watcher_sleep")]
    pub default_hs_monitoring_interval_ms: u64,

    /// Scheduling interval (ms) at which the key-based (external HS) monitoring task is triggered.
    #[serde(default = "default_external_hs_monitoring_interval_ms")]
    pub external_hs_monitoring_interval_ms: u64,

    /// Scheduling interval (ms) at which the user HS resolver task is triggered.
    /// The alias `hs_resolver_sleep` is kept for backward compatibility.
    #[serde(
        default = "default_hs_resolver_interval_ms",
        alias = "hs_resolver_sleep"
    )]
    pub hs_resolver_interval_ms: u64,

    /// Minimum time (ms) before a user's homeserver mapping is re-resolved.
    /// Users whose `HOSTED_BY.resolved_at` is newer than this TTL are skipped.
    #[serde(default = "default_hs_resolver_ttl")]
    pub hs_resolver_ttl: u64,

    /// Initial backoff duration (in seconds) after the first failure of a homeserver
    #[serde(default = "default_initial_backoff_secs")]
    pub initial_backoff_secs: u64,

    /// Maximum backoff duration (in seconds) for a failing homeserver
    #[serde(default = "default_max_backoff_secs")]
    pub max_backoff_secs: u64,

    /// Max file size in bytes (Content-Length check + streaming enforcement).
    /// Rejected files are permanent failures (not retried). Default: 50 MiB.
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,

    #[serde(default = "default_stack")]
    pub stack: StackConfig,

    #[serde(default)]
    pub retry: EventRetryConfig,

    // Moderation
    pub moderation_id: PubkyId,
    pub moderated_tags: Vec<String>,
}

impl Default for WatcherConfig {
    /// The default values are derived from predefined constants
    /// This implementation is not secure as it may panic if the homeserver
    /// identifier fails to parse, but it ensures a valid initial state
    fn default() -> Self {
        let homeserver = PubkyId::try_from(HOMESERVER_PUBKY)
            .expect("Hardcoded homeserver should be a valid pubky id");
        let moderation_id = PubkyId::try_from(DEFAULT_MODERATION_ID)
            .expect("Hardcoded default moderation should be a valid pubky id");
        Self {
            stack: StackConfig::default(),
            homeserver,
            events_limit: DEFAULT_EVENTS_LIMIT,
            key_based_events_limit: DEFAULT_KEY_BASED_EVENTS_LIMIT,
            monitored_homeservers_limit: DEFAULT_MONITORED_HOMESERVERS_LIMIT,
            default_hs_monitoring_interval_ms: DEFAULT_DEFAULT_HS_MONITORING_INTERVAL_MS,
            external_hs_monitoring_interval_ms: DEFAULT_EXTERNAL_HS_MONITORING_INTERVAL_MS,
            hs_resolver_interval_ms: DEFAULT_HS_RESOLVER_INTERVAL_MS,
            hs_resolver_ttl: DEFAULT_HS_RESOLVER_TTL,
            initial_backoff_secs: DEFAULT_INITIAL_BACKOFF_SECS,
            max_backoff_secs: DEFAULT_MAX_BACKOFF_SECS,
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            retry: EventRetryConfig::default(),
            moderation_id,
            moderated_tags: MODERATED_TAGS.iter().map(|s| s.to_string()).collect(),
        }
    }
}

fn default_hs_resolver_interval_ms() -> u64 {
    DEFAULT_HS_RESOLVER_INTERVAL_MS
}

fn default_external_hs_monitoring_interval_ms() -> u64 {
    DEFAULT_EXTERNAL_HS_MONITORING_INTERVAL_MS
}

fn default_key_based_events_limit() -> u16 {
    DEFAULT_KEY_BASED_EVENTS_LIMIT
}

fn deserialize_events_limit<'de, D: Deserializer<'de>>(d: D) -> Result<u16, D::Error> {
    check_limit(u16::deserialize(d)?, "events_limit", MAX_EVENTS_LIMIT)
}

fn deserialize_key_based_events_limit<'de, D: Deserializer<'de>>(d: D) -> Result<u16, D::Error> {
    check_limit(
        u16::deserialize(d)?,
        "key_based_events_limit",
        MAX_KEY_BASED_EVENTS_LIMIT,
    )
}

fn check_limit<E: Error>(val: u16, field: &str, max: u16) -> Result<u16, E> {
    match val {
        0 => Err(E::custom(format!("{field} must be at least 1"))),
        v if v > max => Err(E::custom(format!("{field} ({v}) exceeds max ({max})"))),
        v => Ok(v),
    }
}

fn default_hs_resolver_ttl() -> u64 {
    DEFAULT_HS_RESOLVER_TTL
}

/// Extracts [`WatcherConfig`] from [`DaemonConfig`]
impl From<DaemonConfig> for WatcherConfig {
    fn from(daemon_config: DaemonConfig) -> Self {
        WatcherConfig {
            stack: daemon_config.stack,
            ..daemon_config.watcher
        }
    }
}

#[async_trait]
impl ConfigLoader<WatcherConfig> for WatcherConfig {}

fn default_initial_backoff_secs() -> u64 {
    DEFAULT_INITIAL_BACKOFF_SECS
}

fn default_max_backoff_secs() -> u64 {
    DEFAULT_MAX_BACKOFF_SECS
}

fn default_max_file_size() -> u64 {
    DEFAULT_MAX_FILE_SIZE
}
