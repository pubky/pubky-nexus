use std::net::{IpAddr, Ipv4Addr};
use std::{fmt::Debug, net::SocketAddr};

use super::file::ConfigLoader;
use super::{default_stack, DaemonConfig, StackConfig};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub const NAME: &str = "nexus.api";

pub const DEFAULT_LOCAL_IP: [u8; 4] = [127, 0, 0, 1];
pub const DEFAULT_ICANN_LOCAL_PORT: u16 = 8080;
pub const DEFAULT_PUBKY_LOCAL_PORT: u16 = 8081;

/// Configuration settings for the Nexus Watcher service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub name: String,
    pub public_ip: IpAddr,
    pub public_addr: SocketAddr,
    pub pubky_listen_socket: SocketAddr,
    #[serde(default = "default_stack")]
    pub stack: StackConfig,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            name: String::from(NAME),
            public_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            public_addr: SocketAddr::from((DEFAULT_LOCAL_IP, DEFAULT_ICANN_LOCAL_PORT)),
            pubky_listen_socket: SocketAddr::from((DEFAULT_LOCAL_IP, DEFAULT_PUBKY_LOCAL_PORT)),
            stack: StackConfig::default(),
        }
    }
}

/// Converts a [`DaemonConfig`] into an [`ApiConfig`], extracting only the API-related settings
/// and the shared application stack
impl From<DaemonConfig> for ApiConfig {
    fn from(daemon_config: DaemonConfig) -> Self {
        ApiConfig {
            stack: daemon_config.stack,
            ..daemon_config.api
        }
    }
}

#[async_trait]
impl ConfigLoader<ApiConfig> for ApiConfig {}
