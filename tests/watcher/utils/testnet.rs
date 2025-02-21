use anyhow::Error;
use pubky_testnet::Testnet;
use std::sync::Arc;
use tokio::sync::OnceCell;

static TESTNET_NETWORK: OnceCell<TestnetNetwork> = OnceCell::const_new();

/// Represents a test network for the Distributed Hash Table (DHT).
pub struct TestnetNetwork {
    nodes: Arc<Testnet>,
}

impl TestnetNetwork {
    /// Returns the testnet. This method will initialize the testnet if it hasn't been
    /// already, and will wait for the initialization to complete if another task is doing it.
    pub async fn get() -> Result<Arc<Testnet>, Error> {
        let network = TESTNET_NETWORK
            .get_or_init(|| async {
                let testnet = Testnet::run()
                    .await
                    .expect("Failed to run testnet with hardcoded configurations");
                TestnetNetwork {
                    nodes: Arc::new(testnet),
                }
            })
            .await;
        Ok(network.nodes.clone())
    }
}
