use pubky::errors::{AuthError, BuildError, PkarrError};
use pubky::{CredentialRefreshError, Pubky, PubkyHttpClient};
use std::{sync::Arc, time::Duration};
use thiserror::Error;

use crate::config::NetConfig;
use tracing::debug;

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

    /// A 4xx other than 404, 408 and 429: the server refused this request, so a retry
    /// gets the same answer.
    #[error("Request rejected ({status}): {message}")]
    RequestRejected { status: u16, message: String },

    #[error("Request failed: {message}")]
    RequestFailed { message: String },

    #[error("Pkarr failed: {0}")]
    PkarrFailed(#[from] PkarrError),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(#[from] AuthError),

    /// The SDK failed to refresh a grant credential. Only a signed-in client has one,
    /// and Nexus signs in only in tests, so a retry would not help.
    #[error("Credential refresh failed: {0}")]
    CredentialRefreshFailed(#[from] CredentialRefreshError),

    #[error("Build failed: {0}")]
    BuildFailed(#[from] BuildError),

    #[error("Parse failed: {0}")]
    ParseFailed(#[from] url::ParseError),
}

/// Builds the Pubky SDK client the `[stack.net]` settings describe: for the testnet relay on
/// [`NetConfig::testnet_host`] when [`NetConfig::testnet`] is set, otherwise for mainnet, with
/// every HTTP request, its response body included, bounded by
/// [`NetConfig::pubky_http_request_timeout_secs`].
pub fn build_pubky(net: &NetConfig) -> PubkyClientResult<Arc<Pubky>> {
    build_pubky_with(
        net.pubky_client_testnet_host(),
        net.pubky_client_http_request_timeout(),
    )
}

fn build_pubky_with(
    testnet_host: Option<&str>,
    http_request_timeout: Duration,
) -> PubkyClientResult<Arc<Pubky>> {
    let mode = testnet_host
        .map(|host| format!("testnet with host '{host}'"))
        .unwrap_or_else(|| "mainnet".to_string());
    debug!(
        ?http_request_timeout,
        "Building the Pubky client in {mode} mode"
    );

    // TODO: set pkarr's `minimum_ttl` / `maximum_ttl` and its DHT and relay request
    // timeouts explicitly, as config values; `request_timeout` bounds only the HTTP
    // client, not pkarr.
    let mut client_builder = PubkyHttpClient::builder();
    client_builder.request_timeout(http_request_timeout);

    if let Some(host) = testnet_host {
        client_builder
            .testnet_with_host(host)
            // Force pkarr/mainline DHT to bind an ephemeral local port instead of default behavior
            // We do this to prevent the client DHT from competing with `StaticTestnet` for port 6881
            .pkarr(|p| {
                p.dht(|c| {
                    c.port = Some(0);
                    c
                })
            });
    }

    let client = client_builder
        .build()
        .map_err(PubkyClientError::BuildFailed)?;
    Ok(Arc::new(Pubky::with_client(client)))
}
