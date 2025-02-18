use anyhow::{anyhow, Error};
use log::warn;
use pubky_testnet::Testnet;
use std::sync::Arc;
use tokio::sync::OnceCell;

static TESTNET_NETWORK_SINGLETON: OnceCell<TestnetNetwork> = OnceCell::const_new();

/// Represents a test network for the Distributed Hash Table (DHT).
pub struct TestnetNetwork {
    nodes: Arc<Testnet>,
}

impl TestnetNetwork {
    /// Initializes the test network singleton.
    ///
    /// Sets up the global `TESTNET_NETWORK_SINGLETON` with a new
    /// `TestnetNetwork` instance if it has not already been initialized.
    /// The initialization creates a testnet by running it with hardcoded
    /// configurations.
    pub async fn initialise() -> Result<(), Error> {
        if TESTNET_NETWORK_SINGLETON.get().is_some() {
            return Ok(());
        }

        let testnet = Testnet::run_with_hardcoded_configurations().await?;

        let network = TestnetNetwork {
            nodes: Arc::new(testnet),
        };

        if TESTNET_NETWORK_SINGLETON.set(network).is_err() {
            warn!("DHT Testnet network was already initialized.");
        }

        Ok(())
    }

    /// Retrieves the DHT test network nodes.
    ///
    /// Provides access to the global DHT network's nodes stored in
    /// the `TESTNET_NETWORK_SINGLETON`
    pub fn get_testnet() -> Result<Arc<Testnet>, Error> {
        if let Some(network) = TESTNET_NETWORK_SINGLETON.get() {
            Ok(network.nodes.clone())
        } else {
            Err(anyhow!("DHT testnet network not initialised"))
        }
    }
}
