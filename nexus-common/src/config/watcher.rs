use super::file::ConfigLoader;
use super::{default_stack, DaemonConfig, StackConfig};
use async_trait::async_trait;
use pubky_app_specs::PubkyId;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

pub const NAME: &str = "nexus.watcher";

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

/// Configuration settings for the Nexus Watcher service
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct WatcherConfig {
    pub name: String,
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
    #[serde(default = "default_stack")]
    pub stack: StackConfig,
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
            name: NAME.to_string(),
            stack: StackConfig::default(),
            testnet: TESTNET,
            testnet_host: DEFAULT_TESTNET_HOST.to_string(),
            homeserver,
            events_limit: DEFAULT_EVENTS_LIMIT,
            monitored_homeservers_limit: DEFAULT_MONITORED_HOMESERVERS_LIMIT,
            watcher_sleep: DEFAULT_WATCHER_SLEEP,
            moderation_id,
            moderated_tags: MODERATED_TAGS.iter().map(|s| s.to_string()).collect(),
        }
    }
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
