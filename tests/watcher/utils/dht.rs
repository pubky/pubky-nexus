use tokio::sync::OnceCell;
use pkarr::mainline::Testnet;
use std::sync::Arc;
use anyhow::{anyhow, Error};


static DHT_TESTNET_NETWORK_SINGLETON: OnceCell<TestnetDHTNetwork> = OnceCell::const_new();

/// Represents a test network for the Distributed Hash Table (DHT).
pub struct TestnetDHTNetwork {
    nodes: Arc<Testnet>
}

impl TestnetDHTNetwork {
    /// Initializes the DHT test network singleton.
    ///
    /// Sets up the global `DHT_TESTNET_NETWORK_SINGLETON` with a new
    /// `TestnetDHTNetwork` instance if it has not already been initialized. 
    /// The initialization creates a testnet with a specified capacity.
    pub fn initialise() -> Result<(), Error>{
        if DHT_TESTNET_NETWORK_SINGLETON.get().is_some() {
            return Ok(());
        }
        let testnet = Self {
            nodes: Arc::new(Testnet::new(10)),
        };
        DHT_TESTNET_NETWORK_SINGLETON
            .set(testnet)
            .map_err(|_| anyhow!("Already initiailsed"))?;
        Ok(())
    }

    /// Retrieves the DHT test network nodes.
    ///
    /// Provides access to the global DHT network's nodes stored in
    /// the `DHT_TESTNET_NETWORK_SINGLETON`
    pub fn get_testnet_dht_nodes() -> Result<Arc<Testnet>, Error> {
        if let Some(resolver) = DHT_TESTNET_NETWORK_SINGLETON.get() {
            Ok(resolver.nodes.clone())
        } else {
            Err(anyhow!("DHT testnet network not initialised"))
        }
    }
}