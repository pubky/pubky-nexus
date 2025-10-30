use pubky::{Pubky, PubkyHttpClient};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::OnceCell;
use tracing::debug;

static PUBKY_SINGLETON: OnceCell<Arc<Pubky>> = OnceCell::const_new();

#[derive(Debug, Error)]
pub enum PubkyClientError {
    #[error("PubkyClient not initialized")]
    NotInitialized,

    #[error("Client initialization error: {0}")]
    ClientError(String),
}

pub struct PubkyClient;

impl PubkyClient {
    /// Initializes the `Pubky` singleton.
    ///
    /// - For mainnet, pass `None`.
    /// - For testnet, pass `Some(hostname)` (e.g., "localhost" or "homeserver").
    pub async fn initialise(testnet_host: Option<&str>) -> Result<(), PubkyClientError> {
        PUBKY_SINGLETON
            .get_or_try_init(|| async {
                let mode = testnet_host
                    .map(|host| format!("testnet with host '{host}'"))
                    .unwrap_or_else(|| "mainnet".to_string());
                debug!("Initialising Pubky singleton in {mode} mode");

                let client = match testnet_host {
                    Some(host) => PubkyHttpClient::builder().testnet_with_host(host).build(),
                    None => PubkyHttpClient::new(),
                }
                .map_err(|e| PubkyClientError::ClientError(e.to_string()))?;
                Ok(Arc::new(Pubky::with_client(client)))
            })
            .await
            .map(|_| ())
    }
    /// Retrieves the instance of `Pubky`
    pub fn get() -> Result<Arc<Pubky>, PubkyClientError> {
        PUBKY_SINGLETON
            .get()
            .cloned()
            .ok_or(PubkyClientError::NotInitialized)
    }

    /// Initializes `PUBKY_SINGLETON` with a provided `Pubky` instance.
    ///
    /// # Usage:
    /// - This function is primarily intended for **watcher tests** where a controlled `Pubky` instance
    ///   needs to be injected instead of relying on environment-based initialization
    pub async fn init_from_client(client: Pubky) -> Result<(), PubkyClientError> {
        PUBKY_SINGLETON
            .get_or_try_init(|| async { Ok(Arc::new(client)) })
            .await
            .map(|_| ())
    }
}
