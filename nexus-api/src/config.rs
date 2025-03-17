use std::{fmt::Debug, net::SocketAddr};

use async_trait::async_trait;
use nexus_common::ConfigLoader;
use nexus_common::{default_stack, Config as StackConfig};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub const NAME: &str = "nexus.api";

pub const DEFAULT_HOST: [u8; 4] = [127, 0, 0, 1];
pub const DEFAULT_PORT: u16 = 8080;

/// Configuration settings for the Nexus Watcher service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub name: String,
    pub public_addr: SocketAddr,
    #[serde(default = "default_stack")]
    pub stack: StackConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: String::from(NAME),
            public_addr: SocketAddr::from((DEFAULT_HOST, DEFAULT_PORT)),
            stack: StackConfig::default(),
        }
    }
}

#[async_trait]
impl<T> ConfigLoader<T> for Config where T: DeserializeOwned + Send + Sync + Debug {}
