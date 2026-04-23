use super::file::ConfigLoader;
use super::{default_stack, DaemonConfig, StackConfig};
use async_trait::async_trait;
use pubky_app_specs::PubkyId;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

pub const TESTNET: bool = false;
pub const DEFAULT_TESTNET_HOST: &str = "localhost";
// Testnet homeserver key
pub const HOMESERVER_PUBKY: &str = "8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo";
/// Default for [WatcherConfig::events_limit]
pub const DEFAULT_EVENTS_LIMIT: u32 = 1_000;
/// Default for [WatcherConfig::monitored_homeservers_limit]
pub const DEFAULT_MONITORED_HOMESERVERS_LIMIT: usize = 50;
/// Default for [WatcherConfig::watcher_sleep]
pub const DEFAULT_WATCHER_SLEEP: u64 = 5_000;
/// Default for [WatcherConfig::hs_resolver_sleep]
pub const DEFAULT_HS_RESOLVER_SLEEP: u64 = 10_000;
/// Default for [WatcherConfig::hs_resolver_ttl]: 1 hour in milliseconds
pub const DEFAULT_HS_RESOLVER_TTL: u64 = 3_600_000;
/// Default for [WatcherConfig::initial_backoff_secs]
pub const DEFAULT_INITIAL_BACKOFF_SECS: u64 = 60;
/// Default for [WatcherConfig::max_backoff_secs]
pub const DEFAULT_MAX_BACKOFF_SECS: u64 = 3_600;

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

// Moderation service key
pub const MODERATION_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
// Moderation service key
pub const MODERATED_TAGS: [&str; 6] = [
    "hatespeech",
    "harassement",
    "terrorism",
    "violence",
    "illegal_activities",
    "il_adult_nu_sex_act",
];

/// Retry configuration settings
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
    pub testnet: bool,
    pub testnet_host: String,

    /// Default homeserver. Other homeservers may be ingested in addition, but this one is prioritized.
    pub homeserver: PubkyId,

    /// Maximum number of events to fetch per run from each homeserver
    pub events_limit: u32,

    /// Maximum number of monitored homeservers
    pub monitored_homeservers_limit: usize,

    /// Sleep between every full run (over all monitored homeservers), in milliseconds
    pub watcher_sleep: u64,

    /// Sleep between every run of the user HS resolver periodic task, in milliseconds
    #[serde(default = "default_hs_resolver_sleep")]
    pub hs_resolver_sleep: u64,

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
    #[serde(default = "default_stack")]
    pub stack: StackConfig,

    // Retry configuration
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
        let moderation_id = PubkyId::try_from(MODERATION_ID)
            .expect("Hardcoded moderation should be a valid pubky id");
        Self {
            stack: StackConfig::default(),
            testnet: TESTNET,
            testnet_host: DEFAULT_TESTNET_HOST.to_string(),
            homeserver,
            events_limit: DEFAULT_EVENTS_LIMIT,
            monitored_homeservers_limit: DEFAULT_MONITORED_HOMESERVERS_LIMIT,
            watcher_sleep: DEFAULT_WATCHER_SLEEP,
            hs_resolver_sleep: DEFAULT_HS_RESOLVER_SLEEP,
            hs_resolver_ttl: DEFAULT_HS_RESOLVER_TTL,
            initial_backoff_secs: DEFAULT_INITIAL_BACKOFF_SECS,
            max_backoff_secs: DEFAULT_MAX_BACKOFF_SECS,
            retry: EventRetryConfig::default(),
            moderation_id,
            moderated_tags: MODERATED_TAGS.iter().map(|s| s.to_string()).collect(),
        }
    }
}

fn default_hs_resolver_sleep() -> u64 {
    DEFAULT_HS_RESOLVER_SLEEP
}

fn default_hs_resolver_ttl() -> u64 {
    DEFAULT_HS_RESOLVER_TTL
}

/// Converts a [`DaemonConfig`] into an [`WatcherConfig`], extracting only the Watcher-related settings
/// and the shared application stack
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
