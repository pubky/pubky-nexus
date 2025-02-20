use anyhow::{anyhow, Error};
use pubky_testnet::Testnet;
use std::sync::Arc;
use tokio::sync::OnceCell;

static TESTNET_NETWORK: OnceCell<Arc<Testnet>> = OnceCell::const_new();
pub struct TestnetNetwork {}

impl TestnetNetwork {
    /// Retrieves an instance of the `Testnet` network
    pub async fn get() -> Result<Arc<Testnet>, Error> {
        TESTNET_NETWORK
            .get_or_try_init(|| async {
                let testnet = Testnet::run().await.map_err(|e| e)?;
                Ok(Arc::new(testnet))
            })
            .await
            .map(|arc| arc.clone())
            .map_err(|e: Error| anyhow!("Could not get the testnet network, {:?}", e).into())
    }
}
