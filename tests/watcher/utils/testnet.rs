use anyhow::{anyhow, Error};
use pkarr::PublicKey;
use pubky_homeserver::Homeserver;
use pubky_testnet::Testnet;
use std::sync::Arc;
use tokio::sync::OnceCell;

static TESTNET_NETWORK: OnceCell<Arc<(Testnet, Homeserver)>> = OnceCell::const_new();

/// Represents a test network for the Distributed Hash Table (DHT).
pub struct TestnetNetwork {}

impl TestnetNetwork {
    /// Retrieves or initializes the instance of the testnet network
    /// The testnet network is created using hardcoded configurations
    pub async fn get_homeserver_id() -> Result<PublicKey, Error> {
        TESTNET_NETWORK
            .get_or_try_init(|| async {
                let testnet = Testnet::run_with_hardcoded_configurations()
                    .await
                    .map_err(|e| e)?;
                let homeserver = testnet.run_homeserver().await.map_err(|e| e)?;
                Ok(Arc::new((testnet, homeserver)))
            })
            .await
            .map(|arc| arc.1.public_key())
            .map_err(|e: Error| anyhow!("Could not get the testnet network, {:?}", e).into())
    }
}
