use std::{fmt::Debug, net::SocketAddr};

use async_trait::async_trait;
use nexus_common::db::{Config as StackConfig, ConfigLoader};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub const NAME: &str = "nexus.api";
pub const DEFAULT_HOST: [u8; 4] = [127, 0, 0, 1];
pub const DEFAULT_PORT: u16 = 8080;

/// Configuration settings for the Nexus Watcher service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // TODO: Choose a right name
    pub stack: StackConfig,
    pub public_addr: SocketAddr,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            stack: StackConfig::default(String::from(NAME)),
            public_addr: SocketAddr::from((DEFAULT_HOST, DEFAULT_PORT)),
        }
    }
}

#[async_trait]
impl<T> ConfigLoader<T> for Config where T: DeserializeOwned + Send + Sync + Debug {}
