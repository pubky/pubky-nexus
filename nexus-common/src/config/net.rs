use pubky_app_specs::PubkyId;
use serde::{Deserialize, Serialize};

const DEFAULT_TESTNET_HOST: &str = "localhost";

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
    /// External HS PKs which are forbidden from being indexed.
    #[serde(default)]
    pub external_hs_pk_blacklist: Vec<PubkyId>,
}

impl Default for NetConfig {
    fn default() -> Self {
        Self {
            testnet: false,
            testnet_host: Self::default_testnet_host(),
            external_hs_pk_blacklist: Vec::new(),
        }
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
}
