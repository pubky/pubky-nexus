use std::net::{IpAddr, Ipv4Addr};
use std::{fmt::Debug, net::SocketAddr};

use super::file::ConfigLoader;
use super::{default_stack, DaemonConfig, StackConfig};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub const DEFAULT_LOCAL_IP: [u8; 4] = [127, 0, 0, 1];
pub const DEFAULT_ICANN_LOCAL_PORT: u16 = 8080;
pub const DEFAULT_PUBKY_LOCAL_PORT: u16 = 8081;
/// Default time (in seconds) before a request is aborted with a 408 Request Timeout
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;
/// Default maximum size (in bytes) accepted for a request body
pub const DEFAULT_MAX_BODY_SIZE_BYTES: usize = 1024 * 1024;

const fn default_request_timeout_secs() -> u64 {
    DEFAULT_REQUEST_TIMEOUT_SECS
}

const fn default_max_body_size_bytes() -> usize {
    DEFAULT_MAX_BODY_SIZE_BYTES
}

/// Configuration settings for the Nexus API service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub public_ip: IpAddr,
    pub public_addr: SocketAddr,
    pub pubky_listen_socket: SocketAddr,
    /// Time (in seconds) before a request is aborted with a 408 Request Timeout
    #[serde(default = "default_request_timeout_secs")]
    pub request_timeout_secs: u64,
    /// Maximum size (in bytes) accepted for a request body
    #[serde(default = "default_max_body_size_bytes")]
    pub max_body_size_bytes: usize,
    #[serde(default = "default_stack")]
    pub stack: StackConfig,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            public_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            public_addr: SocketAddr::from((DEFAULT_LOCAL_IP, DEFAULT_ICANN_LOCAL_PORT)),
            pubky_listen_socket: SocketAddr::from((DEFAULT_LOCAL_IP, DEFAULT_PUBKY_LOCAL_PORT)),
            request_timeout_secs: DEFAULT_REQUEST_TIMEOUT_SECS,
            max_body_size_bytes: DEFAULT_MAX_BODY_SIZE_BYTES,
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
