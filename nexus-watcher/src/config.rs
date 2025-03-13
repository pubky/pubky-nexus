use async_trait::async_trait;
use nexus_common::{default_stack, Config as StackConfig, ConfigLoader};
use pubky_app_specs::PubkyId;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

pub const NAME: &str = "nexus.watcher";

pub const TESTNET: bool = false;
pub const HOMESERVER_PUBKY: &str = "8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo";
// Maximum number of events to fetch at once from a homeserver
pub const EVENTS_LIMIT: u32 = 1000;
// Sleep between checks to homeserver
pub const WATCHER_SLEEP: u64 = 5000;

/// Configuration settings for the Nexus Watcher service
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Config {
    pub name: String,
    pub testnet: bool,
    pub homeserver: PubkyId,
    pub events_limit: u32,
    pub watcher_sleep: u64,
    #[serde(default = "default_stack")]
    // TODO: Choose a right name
    pub stack: StackConfig,
}

impl Default for Config {
    /// The default values are derived from predefined constants
    /// This implementation is not secure as it may panic if the homeserver
    /// identifier fails to parse, but it ensures a valid initial state
    fn default() -> Self {
        let homeserver = PubkyId::try_from(HOMESERVER_PUBKY).unwrap();
        Self {
            name: String::from(NAME),
            stack: StackConfig::default(),
            testnet: TESTNET,
            homeserver,
            events_limit: EVENTS_LIMIT,
            watcher_sleep: WATCHER_SLEEP,
        }
    }
}

#[async_trait]
impl<T> ConfigLoader<T> for Config where T: DeserializeOwned + Send + Sync + Debug {}
