use pubky::errors::{AuthError, BuildError, PkarrError, RequestError};
use pubky::{Pubky, PubkyHttpClient, StatusCode};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::OnceCell;
use tracing::debug;

static PUBKY_SINGLETON: OnceCell<Arc<Pubky>> = OnceCell::const_new();

pub type PubkyClientResult<T> = std::result::Result<T, PubkyClientError>;

#[derive(Debug, Error)]
pub enum PubkyClientError {
    #[error("PubkyClient not initialized")]
    NotInitialized,

    #[error("404: {message}")]
    NotFound404 { message: String },

    #[error("429: {message}")]
    TooManyRequests429 { message: String },

    #[error("Server error (5xx): {message}")]
    ServerError5xx { message: String },

    #[error("Request failed: {message}")]
    RequestFailed { message: String },

    #[error("Pkarr failed: {0}")]
    PkarrFailed(#[from] PkarrError),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(#[from] AuthError),

    #[error("Build failed: {0}")]
    BuildFailed(#[from] BuildError),

    #[error("Parse failed: {0}")]
    ParseFailed(#[from] url::ParseError),
}

impl From<pubky::Error> for PubkyClientError {
    fn from(err: pubky::Error) -> Self {
        match err {
            pubky::Error::Request(RequestError::Server { status, message }) => match status {
                StatusCode::NOT_FOUND => Self::NotFound404 { message },
                StatusCode::TOO_MANY_REQUESTS => Self::TooManyRequests429 { message },
                s if s.is_server_error() => Self::ServerError5xx { message },
                _ => Self::RequestFailed { message },
            },
            pubky::Error::Request(RequestError::Transport(e)) => Self::RequestFailed {
                message: e.to_string(),
            },
            pubky::Error::Request(
                RequestError::Validation { message } | RequestError::DecodeJson { message },
            ) => Self::RequestFailed { message },

            pubky::Error::Pkarr(e) => e.into(),
            pubky::Error::Authentication(e) => e.into(),
            pubky::Error::Build(e) => e.into(),
            pubky::Error::Parse(e) => e.into(),
        }
    }
}

pub struct PubkyConnector;

impl PubkyConnector {
    /// Initializes the `Pubky` singleton.
    ///
    /// - For mainnet, pass `None`.
    /// - For testnet, pass `Some(hostname)` (e.g., "localhost" or "homeserver").
    pub async fn initialise(testnet_host: Option<&str>) -> PubkyClientResult<()> {
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
                .map_err(|e| PubkyClientError::from(pubky::Error::from(e)))?;
                Ok(Arc::new(Pubky::with_client(client)))
            })
            .await
            .map(|_| ())
    }
    /// Retrieves the instance of `Pubky`
    pub fn get() -> PubkyClientResult<Arc<Pubky>> {
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
    pub async fn init_from(sdk: Pubky) -> PubkyClientResult<()> {
        PUBKY_SINGLETON
            .get_or_try_init(|| async { Ok(Arc::new(sdk)) })
            .await
            .map(|_| ())
    }
}
