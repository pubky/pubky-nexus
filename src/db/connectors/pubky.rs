use pubky::Client;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::OnceCell;

use crate::{events::error::EventProcessorError, types::DynError, Config};

static PUBKY_CLIENT_SINGLETON: OnceCell<Arc<Client>> = OnceCell::const_new();

#[derive(Debug, Error)]
pub enum PubkyClientError {
    #[error("PubkyClient not initialized")]
    NotInitialized,

    #[error("Client initialization error: {0}")]
    ClientError(String),
}

pub struct PubkyClient;

impl PubkyClient {
    /// Retrieves an instance of the `PubkyClient`
    ///
    /// # Behavior:
    /// - Determines whether to create a **testnet** or **mainnet** client
    pub async fn get() -> Result<Arc<Client>, DynError> {
        PUBKY_CLIENT_SINGLETON
            .get_or_try_init(|| async {
                let client = match Config::homeserver_network() {
                    true => Client::builder()
                        .testnet()
                        .build()
                        .map_err(|e| PubkyClientError::ClientError(e.to_string()))?,
                    false => Client::builder()
                        .build()
                        .map_err(|e| PubkyClientError::ClientError(e.to_string()))?,
                };

                Ok(Arc::new(client))
            })
            .await
            .cloned()
            .map_err(|e: PubkyClientError| {
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
    pub async fn init_from_client(client: Client) -> Result<(), PubkyClientError> {
        PUBKY_CLIENT_SINGLETON
            .get_or_try_init(|| async { Ok(Arc::new(client)) })
            .await
            .map(|_| ())
    }
}
