use std::time::Duration;

use pubky_social_specs::PubkyId;
use serde::{de::Error, Deserialize, Deserializer, Serialize};

const DEFAULT_TESTNET_HOST: &str = "localhost";
const DEFAULT_PUBKY_HTTP_REQUEST_TIMEOUT_SECS: u64 = 300;

const fn default_pubky_http_request_timeout_secs() -> u64 {
    DEFAULT_PUBKY_HTTP_REQUEST_TIMEOUT_SECS
}

/// pkarr's own defaults, except the cache size (see [`NetConfig::pkarr_cache_size`]).
const DEFAULT_PKARR_MINIMUM_TTL_SECS: u32 = pubky::pkarr::DEFAULT_MINIMUM_TTL;
const DEFAULT_PKARR_MAXIMUM_TTL_SECS: u32 = pubky::pkarr::DEFAULT_MAXIMUM_TTL;
const DEFAULT_PKARR_DHT_REQUEST_TIMEOUT_SECS: u64 = 2;
const DEFAULT_PKARR_RELAY_REQUEST_TIMEOUT_SECS: u64 = 5;
const DEFAULT_PKARR_CACHE_SIZE: usize = 10_000;

const fn default_pkarr_minimum_ttl_secs() -> u32 {
    DEFAULT_PKARR_MINIMUM_TTL_SECS
}
const fn default_pkarr_maximum_ttl_secs() -> u32 {
    DEFAULT_PKARR_MAXIMUM_TTL_SECS
}
const fn default_pkarr_dht_request_timeout_secs() -> u64 {
    DEFAULT_PKARR_DHT_REQUEST_TIMEOUT_SECS
}
const fn default_pkarr_relay_request_timeout_secs() -> u64 {
    DEFAULT_PKARR_RELAY_REQUEST_TIMEOUT_SECS
}
const fn default_pkarr_cache_size() -> usize {
    DEFAULT_PKARR_CACHE_SIZE
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
        default = "default_pubky_http_request_timeout_secs",
        deserialize_with = "deserialize_nonzero_pubky_http_request_timeout_secs"
    )]
    pub pubky_http_request_timeout_secs: u64,
    /// Seconds a cached PKDNS record counts as fresh at least, whatever TTL it carries.
    /// A cached record past its TTL is looked up again; while the lookup fails, the
    /// homeserver resolver keeps using it.
    #[serde(default = "default_pkarr_minimum_ttl_secs")]
    pub pkarr_minimum_ttl_secs: u32,
    /// Seconds a cached PKDNS record counts as fresh at most. Raised to
    /// [Self::pkarr_minimum_ttl_secs] when lower.
    #[serde(default = "default_pkarr_maximum_ttl_secs")]
    pub pkarr_maximum_ttl_secs: u32,
    /// Seconds a PKDNS lookup waits for the DHT.
    #[serde(
        default = "default_pkarr_dht_request_timeout_secs",
        deserialize_with = "deserialize_nonzero_secs"
    )]
    pub pkarr_dht_request_timeout_secs: u64,
    /// Seconds a PKDNS lookup waits for each relay.
    #[serde(
        default = "default_pkarr_relay_request_timeout_secs",
        deserialize_with = "deserialize_nonzero_secs"
    )]
    pub pkarr_relay_request_timeout_secs: u64,
    /// PKDNS records kept in memory. It should exceed the number of known users plus
    /// their homeservers: a record evicted from the cache cannot stand in for a failed
    /// lookup.
    #[serde(default = "default_pkarr_cache_size")]
    pub pkarr_cache_size: usize,
    /// External HS PKs which are forbidden from being indexed.
    #[serde(default)]
    pub external_hs_pk_blacklist: Vec<PubkyId>,
}

impl Default for NetConfig {
    fn default() -> Self {
        Self {
            testnet: false,
            testnet_host: Self::default_testnet_host(),
            pubky_http_request_timeout_secs: DEFAULT_PUBKY_HTTP_REQUEST_TIMEOUT_SECS,
            pkarr_minimum_ttl_secs: DEFAULT_PKARR_MINIMUM_TTL_SECS,
            pkarr_maximum_ttl_secs: DEFAULT_PKARR_MAXIMUM_TTL_SECS,
            pkarr_dht_request_timeout_secs: DEFAULT_PKARR_DHT_REQUEST_TIMEOUT_SECS,
            pkarr_relay_request_timeout_secs: DEFAULT_PKARR_RELAY_REQUEST_TIMEOUT_SECS,
            pkarr_cache_size: DEFAULT_PKARR_CACHE_SIZE,
            external_hs_pk_blacklist: Vec::new(),
        }
    }
}

fn deserialize_nonzero_pubky_http_request_timeout_secs<'de, D>(
    deserializer: D,
) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    match u64::deserialize(deserializer)? {
        0 => Err(D::Error::custom(
            "pubky_http_request_timeout_secs must be at least 1 second",
        )),
        timeout => Ok(timeout),
    }
}

fn deserialize_nonzero_secs<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    match u64::deserialize(deserializer)? {
        0 => Err(D::Error::custom(
            "a pkarr timeout must be at least 1 second",
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
        Duration::from_secs(self.pubky_http_request_timeout_secs)
    }
}
