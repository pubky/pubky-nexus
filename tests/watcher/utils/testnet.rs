use anyhow::{anyhow, Error};
use pubky_testnet::Testnet;
use std::sync::Arc;
use tokio::sync::OnceCell;

static TESTNET_NETWORK: OnceCell<Arc<Testnet>> = OnceCell::const_new();

/// Represents a test network for the Distributed Hash Table (DHT).
pub struct TestnetNetwork {}

impl TestnetNetwork {
    /// Retrieves or initializes the instance of the testnet network
    /// The testnet network is created using hardcoded configurations
    pub async fn get() -> Result<Arc<Testnet>, Error> {
        TESTNET_NETWORK
            .get_or_try_init(|| async {
                let testnet = Testnet::run_with_hardcoded_configurations().await
                    .map_err(|e| e)?;
                Ok(Arc::new(testnet))
            })
            .await
            .map(|arc| arc.clone())
            .map_err(|e: Error| anyhow!("Could not get the testnet network, {:?}", e).into())
    }
}
