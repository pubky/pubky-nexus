use crate::Config;
use mainline::Testnet;
use pubky::Client;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::OnceCell;

static PUBKY_CONNECTOR_SINGLETON: OnceCell<Arc<Client>> = OnceCell::const_new();

#[derive(Debug, Error)]
pub enum PubkyConnectorError {
    #[error("PubkyConnector has already been initialized")]
    AlreadyInitialized,

    #[error("PubkyConnector not initialized")]
    NotInitialized,

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Client initialization error: {0}")]
    ClientError(String),
}

pub struct PubkyConnector;

impl PubkyConnector {
    /// Initializes the PubkyConnector singleton with the given configuration
    pub async fn initialise(
        config: &Config,
        testnet: Option<&Testnet>,
    ) -> Result<(), PubkyConnectorError> {
        PUBKY_CONNECTOR_SINGLETON
            .get_or_try_init(|| async {
                let pubky_client = match testnet {
                    Some(testnet) => Client::builder()
                        .testnet(testnet)
                        .build()
                        .map_err(|e| PubkyConnectorError::ClientError(e.to_string()))?,
                    None => match config.testnet {
                        true => Client::testnet()
                            .map_err(|e| PubkyConnectorError::ClientError(e.to_string()))?,
                        false => Client::new()
                            .map_err(|e| PubkyConnectorError::ClientError(e.to_string()))?,
                    },
                };

                Ok(Arc::new(pubky_client))
            })
            .await
            .map(|_| ())
    }

    /// Retrieves the shared Client connection.
    pub fn get_pubky_client() -> Result<Arc<Client>, PubkyConnectorError> {
        PUBKY_CONNECTOR_SINGLETON
            .get()
            .cloned()
            .ok_or(PubkyConnectorError::NotInitialized)
    }
}
