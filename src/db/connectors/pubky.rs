use log::debug;
use pubky::Client;
use std::{env, sync::Arc};
use thiserror::Error;
use tokio::sync::OnceCell;

use crate::{events::error::EventProcessorError, types::DynError};

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
                let client = match get_homeserver_network_from_env() {
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
            .map(|arc| arc.clone())
            .map_err(|e: PubkyConnectorError| EventProcessorError::PubkyClientError {
                message: format!("{}", e),
            }
            .into())
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

/// Retrieves the `TESTNET` environment variable and determines whether the homeserver 
/// network should operate in test mode or mainnet
///
/// - If `TESTNET` is set to an **invalid value** (e.g., `"123"`, `"yes"`), it defaults to `false`
fn get_homeserver_network_from_env() -> bool {
    env::var("TESTNET")
        .unwrap_or_else(|_| {
            debug!("TESNET env it is not set, defaulting to false...");
            "false".to_string()
        })
        .parse()
        .unwrap_or(false)
}
