use std::net::{IpAddr, Ipv4Addr};
use std::{fmt::Debug, net::SocketAddr};

use super::file::ConfigLoader;
use super::{default_stack, DaemonConfig, StackConfig};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub const DEFAULT_LOCAL_IP: [u8; 4] = [127, 0, 0, 1];
pub const DEFAULT_ICANN_LOCAL_PORT: u16 = 8080;
pub const DEFAULT_PUBKY_LOCAL_PORT: u16 = 8081;
/// Default time (in seconds) before a request is aborted with a 408 Request Timeout.
/// Values below 1 are clamped to 1; setting this to 0 causes every request to time out
/// immediately (TimeoutLayer resolves on the very first poll, before any handler runs),
/// effectively taking the API offline.
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;
/// Default maximum size (in bytes) accepted for a request body
pub const DEFAULT_MAX_BODY_SIZE_BYTES: usize = 1024 * 1024;

const fn default_request_timeout_secs() -> u64 {
    DEFAULT_REQUEST_TIMEOUT_SECS
}

const fn default_max_body_size_bytes() -> usize {
    DEFAULT_MAX_BODY_SIZE_BYTES
}

/// Per-bucket rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct RateLimitBucketConfig {
    /// Maximum requests per minute
    pub rate: u32,
    /// Maximum burst size
    pub burst: u32,
}

fn default_default_bucket() -> RateLimitBucketConfig {
    RateLimitBucketConfig {
        rate: 300,
        burst: 50,
    }
}

fn default_expensive_bucket() -> RateLimitBucketConfig {
    RateLimitBucketConfig { rate: 20, burst: 5 }
}

/// Rate limiting configuration for the API
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct RateLimitConfig {
    /// Whether rate limiting is enabled (default: false for permissiveness behind NATs)
    #[serde(default)]
    pub enabled: bool,
    /// Default bucket config for standard endpoints
    #[serde(default = "default_default_bucket")]
    pub default_bucket: RateLimitBucketConfig,
    /// Expensive bucket config for high-cost endpoints
    #[serde(default = "default_expensive_bucket")]
    pub expensive_bucket: RateLimitBucketConfig,
    /// When true, forwarded-IP headers (X-Forwarded-For / X-Real-IP) are trusted for
    /// real-IP extraction. Only enable behind a known reverse proxy; never in direct-to-internet
    /// deployments (clients can spoof these headers).
    #[serde(default)]
    pub trust_proxy_headers: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_bucket: default_default_bucket(),
            expensive_bucket: default_expensive_bucket(),
            trust_proxy_headers: false,
        }
    }
}

/// Configuration settings for the Nexus API service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub public_ip: IpAddr,
    pub public_addr: SocketAddr,
    pub pubky_listen_socket: SocketAddr,
    /// Time (in seconds) before a request is aborted with a 408 Request Timeout.
    /// Values below 1 are clamped to 1; see [`DEFAULT_REQUEST_TIMEOUT_SECS`].
    #[serde(default = "default_request_timeout_secs")]
    pub request_timeout_secs: u64,
    /// Maximum size (in bytes) accepted for a request body
    #[serde(default = "default_max_body_size_bytes")]
    pub max_body_size_bytes: usize,
    #[serde(default = "default_stack")]
    pub stack: StackConfig,
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
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
            rate_limit: RateLimitConfig::default(),
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
