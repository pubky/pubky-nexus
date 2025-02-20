use pubky::Client;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::OnceCell;

use crate::{events::error::EventProcessorError, types::DynError, Config};

static PUBKY_CONNECTOR_SINGLETON: OnceCell<Arc<Client>> = OnceCell::const_new();

#[derive(Debug, Error)]
pub enum PubkyConnectorError {
    #[error("PubkyConnector not initialized")]
    NotInitialized,

    #[error("Client initialization error: {0}")]
    ClientError(String),
}

pub struct PubkyConnector;

impl PubkyConnector {
    /// Retrieves an instance of the `PubkyClient`
    ///
    /// # Behavior:
    /// - Determines whether to create a **testnet** or **mainnet** client
    pub async fn get_pubky_client() -> Result<Arc<Client>, DynError> {
        PUBKY_CONNECTOR_SINGLETON
            .get_or_try_init(|| async {
                let client = match Config::homeserver_network() {
                    true => Client::builder()
                        .testnet()
                        .build()
                        .map_err(|e| PubkyConnectorError::ClientError(e.to_string()))?,
                    false => Client::builder()
                        .build()
                        .map_err(|e| PubkyConnectorError::ClientError(e.to_string()))?,
                };

                Ok(Arc::new(client))
            })
            .await
            .cloned()
            .map_err(|e: PubkyConnectorError| {
                EventProcessorError::PubkyClientError {
                    message: format!("{}", e),
                }
                .into()
            })
    }

    /// Initializes the `PUBKY_CONNECTOR_SINGLETON` with a provided `Client` instance.
    ///
    /// # Usage:
    /// - This function is primarily intended for **watcher tests** where a controlled `Client` instance
    ///   needs to be injected instead of relying on environment-based initialization
    pub async fn init_from_client(client: Client) -> Result<(), PubkyConnectorError> {
        PUBKY_CONNECTOR_SINGLETON
            .get_or_try_init(|| async { Ok(Arc::new(client)) })
            .await
            .map(|_| ())
    }
}
