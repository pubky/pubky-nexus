use once_cell::sync::OnceCell;
use pubky::PubkyClient;
use std::fmt;
use std::sync::{Arc, Mutex};

pub struct PubkyConnector {
    client: OnceCell<Arc<Mutex<PubkyClient>>>,
}

impl PubkyConnector {
    pub fn new() -> Self {
        Self {
            client: OnceCell::new(),
        }
    }

    pub async fn connect(&self, client: PubkyClient) -> Result<(), Box<dyn std::error::Error>> {
        self.client
            .set(Arc::new(Mutex::new(client)))
            .map_err(|_| "Failed to set Pubky client instance")?;
        Ok(())
    }

    pub async fn new_connection(client: PubkyClient) -> Result<Self, Box<dyn std::error::Error>> {
        let pubky_connector = PubkyConnector::new();
        pubky_connector.connect(client).await?;
        Ok(pubky_connector)
    }

    pub fn client(&self) -> Arc<Mutex<PubkyClient>> {
        self.client.get().expect("Not connected to Pubky").clone()
    }
}

impl fmt::Debug for PubkyConnector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PubkyConnector")
            .field("client", &"Pubky client instance")
            .finish()
    }
}

pub async fn get_pubky_client(
) -> Result<Arc<Mutex<PubkyClient>>, Box<dyn std::error::Error + Send + Sync>> {
    let pubky_client = PUBKY_CONNECTOR
        .get()
        .ok_or("PubkyConnector not initialized")?
        .client();
    Ok(pubky_client)
}

pub static PUBKY_CONNECTOR: OnceCell<PubkyConnector> = OnceCell::new();
