use super::file::ConfigLoader;
use super::{default_stack, DaemonConfig, StackConfig};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, net::SocketAddr};

pub const NAME: &str = "nexus.api";

pub const DEFAULT_HOST: [u8; 4] = [127, 0, 0, 1];
pub const DEFAULT_PORT: u16 = 8080;

/// Configuration settings for the Nexus Watcher service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub name: String,
    pub public_addr: SocketAddr,
    #[serde(default = "default_stack")]
    pub stack: StackConfig,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            name: String::from(NAME),
            public_addr: SocketAddr::from((DEFAULT_HOST, DEFAULT_PORT)),
            stack: StackConfig::default(),
        }
    }
}

/// Converts a [`DaemonConfig`] into an [`ApiConfig`], extracting only the API-related settings
/// and the shared application stack
impl From<DaemonConfig> for ApiConfig {
    fn from(daemon_config: DaemonConfig) -> Self {
        ApiConfig {
            name: daemon_config.api.name,
            public_addr: daemon_config.api.public_addr,
            stack: daemon_config.stack,
        }
    }
}

#[async_trait]
impl ConfigLoader<ApiConfig> for ApiConfig {}
