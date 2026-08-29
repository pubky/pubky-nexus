use std::time::Duration;

use pubky_app_specs::PubkyId;
use serde::{de::Error, Deserialize, Deserializer, Serialize};

const DEFAULT_TESTNET_HOST: &str = "localhost";
const DEFAULT_PUBKY_HTTP_REQUEST_TIMEOUT_SECS: u64 = 30;

const fn default_pubky_http_request_timeout() -> u64 {
    DEFAULT_PUBKY_HTTP_REQUEST_TIMEOUT_SECS
}

/// Shared Pubky network settings for the Nexus stack (`[stack.net]`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetConfig {
    /// When true, the Pubky SDK client targets a local testnet relay at [Self::testnet_host].
    #[serde(default)]
    pub testnet: bool,
    /// Testnet relay hostname (e.g. `"localhost"` or a Docker service name).
    /// Only used when [Self::testnet] is true.
    #[serde(default = "NetConfig::default_testnet_host")]
    pub testnet_host: String,
    /// Total request timeout in seconds for every HTTP request made by the shared Pubky client.
    /// This includes reading the complete response body, so the default accommodates
    /// downloads up to [`crate::DEFAULT_MAX_FILE_SIZE`].
    #[serde(
        default = "default_pubky_http_request_timeout",
        deserialize_with = "deserialize_nonzero_pubky_http_request_timeout"
    )]
    pub pubky_http_request_timeout: u64,
    /// External HS PKs which are forbidden from being indexed.
    #[serde(default)]
    pub external_hs_pk_blacklist: Vec<PubkyId>,
}

impl Default for NetConfig {
    fn default() -> Self {
        Self {
            testnet: false,
            testnet_host: Self::default_testnet_host(),
            pubky_http_request_timeout: DEFAULT_PUBKY_HTTP_REQUEST_TIMEOUT_SECS,
            external_hs_pk_blacklist: Vec::new(),
        }
    }
}

fn deserialize_nonzero_pubky_http_request_timeout<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    match u64::deserialize(deserializer)? {
        0 => Err(D::Error::custom(
            "pubky_http_request_timeout must be at least 1 second",
        )),
        timeout => Ok(timeout),
    }
}

impl NetConfig {
    fn default_testnet_host() -> String {
        DEFAULT_TESTNET_HOST.to_string()
    }

    /// Returns the testnet relay hostname for [`PubkyConnector::initialise`]
    pub fn pubky_client_testnet_host(&self) -> Option<&str> {
        self.testnet.then_some(self.testnet_host.as_str())
    }

    /// Returns the total request deadline used to initialize the shared Pubky client.
    pub fn pubky_client_http_request_timeout(&self) -> Duration {
        Duration::from_secs(self.pubky_http_request_timeout)
    }
}
