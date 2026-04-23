use pubky::{Pubky, PubkyHttpClient, StatusCode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::OnceCell;
use tracing::debug;

static PUBKY_SINGLETON: OnceCell<Arc<Pubky>> = OnceCell::const_new();

#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum PubkyClientError {
    #[error("PubkyClient not initialized")]
    NotInitialized,

    #[error("404: {message}")]
    NotFound404 { message: String },

    #[error("Server error (5xx): {message}")]
    ServerError5xx { message: String },

    #[error("Request failed: {message}")]
    RequestFailed { message: String },

    #[error("Pkarr failed: {message}")]
    PkarrFailed { message: String },

    #[error("Authentication failed: {message}")]
    AuthenticationFailed { message: String },

    #[error("Build failed: {message}")]
    BuildFailed { message: String },

    #[error("Parse failed: {message}")]
    ParseFailed { message: String },
}

impl From<pubky::Error> for PubkyClientError {
    fn from(err: pubky::Error) -> Self {
        match err {
            pubky::Error::Request(req_err) => match req_err {
                pubky::errors::RequestError::Server { status, message } => {
                    if status == StatusCode::NOT_FOUND {
                        Self::NotFound404 { message }
                    } else if status.is_server_error() {
                        Self::ServerError5xx { message }
                    } else {
                        Self::RequestFailed { message }
                    }
                }
                pubky::errors::RequestError::Transport(err) => Self::RequestFailed {
                    message: err.to_string(),
                },
                pubky::errors::RequestError::Validation { message } => {
                    Self::RequestFailed { message }
                }
                pubky::errors::RequestError::DecodeJson { message } => {
                    Self::RequestFailed { message }
                }
            },
            pubky::Error::Pkarr(pkarr_err) => Self::PkarrFailed {
                message: pkarr_err.to_string(),
            },
            pubky::Error::Authentication(auth_err) => Self::AuthenticationFailed {
                message: auth_err.to_string(),
            },
            pubky::Error::Build(build_err) => Self::BuildFailed {
                message: build_err.to_string(),
            },
            pubky::Error::Parse(parse_err) => Self::ParseFailed {
                message: parse_err.to_string(),
            },
        }
    }
}

impl PubkyClientError {
    /// Returns true if this error is a 404 (content not found)
    pub fn is_404(&self) -> bool {
        matches!(self, Self::NotFound404 { .. })
    }

    /// Returns true if this error is transient and worth retrying
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::ServerError5xx { .. } | Self::RequestFailed { .. } | Self::PkarrFailed { .. }
        )
    }
}

pub struct PubkyConnector;

impl PubkyConnector {
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
                    Some(host) => PubkyHttpClient::builder()
                        .testnet_with_host(host)
                        // Force pkarr/mainline DHT to bind an ephemeral local port instead of default behavior
                        // We do this to prevent the client DHT from competing with `StaticTestnet` for port 6881
                        .pkarr(|p| p.dht(|d| d.port(0)))
                        .build(),
                    None => PubkyHttpClient::new(),
                }
                .map_err(|e| PubkyClientError::BuildFailed {
                    message: e.to_string(),
                })?;
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
    pub async fn init_from(sdk: Pubky) -> Result<(), PubkyClientError> {
        PUBKY_SINGLETON
            .get_or_try_init(|| async { Ok(Arc::new(sdk)) })
            .await
            .map(|_| ())
    }
}
