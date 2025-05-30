use super::file::ConfigLoader;
use super::{default_stack, DaemonConfig, StackConfig};
use async_trait::async_trait;
use pubky_app_specs::PubkyId;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

pub const NAME: &str = "nexus.watcher";

pub const TESTNET: bool = false;
// Testnet homeserver key
pub const HOMESERVER_PUBKY: &str = "8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo";
// Maximum number of events to fetch at once from a homeserver
pub const EVENTS_LIMIT: u32 = 1000;
// Sleep between checks to homeserver
pub const WATCHER_SLEEP: u64 = 5000;
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
    pub homeserver: PubkyId,
    pub events_limit: u32,
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
            name: String::from(NAME),
            stack: StackConfig::default(),
            testnet: TESTNET,
            homeserver,
            events_limit: EVENTS_LIMIT,
            watcher_sleep: WATCHER_SLEEP,
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
            name: daemon_config.watcher.name,
            testnet: daemon_config.watcher.testnet,
            homeserver: daemon_config.watcher.homeserver,
            events_limit: daemon_config.watcher.events_limit,
            watcher_sleep: daemon_config.watcher.watcher_sleep,
            stack: daemon_config.stack,
            moderation_id: daemon_config.watcher.moderation_id,
            moderated_tags: daemon_config.watcher.moderated_tags,
        }
    }
}

#[async_trait]
impl ConfigLoader<WatcherConfig> for WatcherConfig {}
