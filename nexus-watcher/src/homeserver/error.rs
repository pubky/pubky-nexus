use std::fmt::Display;

use pubky::errors::RequestError;
use pubky::PublicKey;
use reqwest::StatusCode;

use super::fetch::{format_error_body, MAX_ERROR_BODY};

/// A failed homeserver call, made through [`WatcherClient`](super::WatcherClient) or another
/// implementation of the capability traits.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{op:?} {hs}: {kind}")]
pub struct HsError {
    /// ID (z-base-32 key) of the homeserver the call went to, or for a resource or blob,
    /// its owner's key. Not a [`PublicKey`], which also holds the decoded key and would make
    /// every error several times larger.
    pub hs: String,
    pub op: HsOp,
    pub kind: HsErrorKind,
}

impl HsError {
    pub fn new(hs: &PublicKey, op: HsOp, kind: HsErrorKind) -> Self {
        Self {
            hs: hs.z32(),
            op,
            kind,
        }
    }
}

/// The homeserver operation a call made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HsOp {
    PollEvents,
    UserEvents,
    GetResource,
    GetBlob,
}

/// Why a homeserver call failed.
///
/// Homeserver-supplied text (response bodies, transport messages) is capped at
/// [`MAX_ERROR_BODY`] bytes when a kind is built through its constructors, before
/// it can reach a log line or a span.
#[derive(Debug, Clone, thiserror::Error)]
pub enum HsErrorKind {
    #[error("HTTP 404: {0}")]
    NotFound(String),
    #[error("HTTP 429: {0}")]
    RateLimited(String),
    #[error("HTTP {0}: {1}")]
    ServerError(u16, String),
    /// Any other non-2xx status.
    #[error("HTTP {0}: {1}")]
    Rejected(u16, String),
    #[error("timed out")]
    Timeout,
    #[error("transport failed: {0}")]
    Transport(String),
    #[error("{got} bytes, over the {max} byte limit")]
    TooLarge { got: u64, max: u64 },
    /// The SDK refused the request, could not decode the answer, or could not resolve a
    /// key: neither a status nor a transport failure. A failed fetch is retried; a failed
    /// event-stream request skips its user for the run, cursor kept, and unlike a transport
    /// failure never stops the run.
    #[error("sdk: {0}")]
    Sdk(String),
    /// A request that could not be built, e.g. from a malformed homeserver URL.
    #[error("malformed: {0}")]
    Malformed(String),
}

impl HsErrorKind {
    /// Classifies a non-2xx status, keeping at most [`MAX_ERROR_BODY`] bytes of its body.
    pub fn from_status(status: StatusCode, body: &[u8]) -> Self {
        let body = format_error_body(body, MAX_ERROR_BODY);
        match status {
            StatusCode::NOT_FOUND => Self::NotFound(body),
            StatusCode::TOO_MANY_REQUESTS => Self::RateLimited(body),
            s if s.is_server_error() => Self::ServerError(s.as_u16(), body),
            s => Self::Rejected(s.as_u16(), body),
        }
    }

    /// A failure to reach the homeserver or to read its response.
    pub fn transport(error: impl Display) -> Self {
        Self::Transport(capped(error))
    }

    pub fn malformed(error: impl Display) -> Self {
        Self::Malformed(capped(error))
    }

    pub fn from_reqwest(error: &reqwest::Error) -> Self {
        match error.is_timeout() {
            true => Self::Timeout,
            false => Self::transport(error),
        }
    }

    /// Classifies an SDK error so it is retried, or not, as `PubkyClientError::from`
    /// classifies it.
    ///
    /// A status lands by its code and a transport failure stays one. A validation, JSON
    /// decode or PKARR failure, which that conversion makes a retried `RequestFailed`
    /// (`PkarrFailed` for PKARR), is an [`HsErrorKind::Sdk`] failure, treated the same
    /// way. An error building the request (`Parse`, `Build`, `Authentication`) is
    /// malformed, and permanent. So is a failed grant credential refresh
    /// (`CredentialRefresh`): only a signed-in client refreshes one, and Nexus signs in
    /// only in tests.
    pub fn from_sdk(error: pubky::Error) -> Self {
        match error {
            pubky::Error::Request(RequestError::Server { status, message }) => {
                Self::from_status(status, message.as_bytes())
            }
            pubky::Error::Request(RequestError::Transport(e)) => Self::from_reqwest(&e),
            pubky::Error::Request(
                RequestError::Validation { .. } | RequestError::DecodeJson { .. },
            )
            | pubky::Error::Pkarr(_) => Self::Sdk(capped(error)),
            pubky::Error::Parse(_)
            | pubky::Error::Authentication(_)
            | pubky::Error::CredentialRefresh(_)
            | pubky::Error::Build(_) => Self::malformed(error),
        }
    }
}

/// `error`'s message, cut to [`MAX_ERROR_BODY`] bytes.
fn capped(error: impl Display) -> String {
    format_error_body(error.to_string().as_bytes(), MAX_ERROR_BODY)
}

/// A failing body read gives a non-timeout `reqwest::Error`, as an unreachable homeserver
/// or a reset connection does.
#[cfg(test)]
pub(crate) async fn transport_error() -> reqwest::Error {
    let body = reqwest::Body::wrap_stream(futures::stream::iter(vec![Err::<bytes::Bytes, _>(
        std::io::Error::other("connection reset"),
    )]));
    reqwest::Response::from(http::Response::new(body))
        .bytes()
        .await
        .unwrap_err()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pubky::errors::{AuthError, PkarrError};
    use pubky::CredentialRefreshError;

    /// A `reqwest::Error` that `is_timeout()`: a request to a local socket nothing answers
    /// on, with a timeout shorter than its patience.
    async fn timeout_error() -> reqwest::Error {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(50))
            .no_proxy()
            .build()
            .unwrap();
        let error = client.get(url).send().await.unwrap_err();
        assert!(error.is_timeout(), "{error:?}");
        error
    }

    /// SDK errors are classified so that statuses land by their code, validation failures
    /// are retried like failed requests, and errors building the request never.
    #[test]
    fn sdk_errors_are_classified() {
        let server = |status: u16| {
            HsErrorKind::from_sdk(pubky::Error::from(RequestError::Server {
                status: StatusCode::from_u16(status).unwrap(),
                message: "nope".into(),
            }))
        };
        assert!(matches!(server(404), HsErrorKind::NotFound(_)));
        assert!(matches!(server(429), HsErrorKind::RateLimited(_)));
        assert!(matches!(server(503), HsErrorKind::ServerError(503, _)));
        assert!(matches!(server(400), HsErrorKind::Rejected(400, _)));

        let validation = HsErrorKind::from_sdk(pubky::Error::from(RequestError::Validation {
            message: "Cannot subscribe to more than 50 users".into(),
        }));
        assert!(matches!(validation, HsErrorKind::Sdk(_)), "{validation:?}");
        let decode = HsErrorKind::from_sdk(pubky::Error::from(RequestError::DecodeJson {
            message: "expected value at line 1".into(),
        }));
        assert!(matches!(decode, HsErrorKind::Sdk(_)), "{decode:?}");
        let pkarr = HsErrorKind::from_sdk(pubky::Error::from(PkarrError::InvalidRecord(
            "no homeserver record".into(),
        )));
        assert!(matches!(pkarr, HsErrorKind::Sdk(_)), "{pkarr:?}");

        let parse_error = reqwest::Url::parse("https://").unwrap_err();
        let parse = HsErrorKind::from_sdk(pubky::Error::from(parse_error));
        assert!(matches!(parse, HsErrorKind::Malformed(_)), "{parse:?}");
        let auth = HsErrorKind::from_sdk(pubky::Error::from(AuthError::Validation(
            "missing token".into(),
        )));
        assert!(matches!(auth, HsErrorKind::Malformed(_)), "{auth:?}");
        let refresh = HsErrorKind::from_sdk(pubky::Error::from(CredentialRefreshError::Request {
            status: None,
        }));
        assert!(matches!(refresh, HsErrorKind::Malformed(_)), "{refresh:?}");
    }

    #[test]
    fn statuses_are_classified() {
        let kind =
            |status: u16| HsErrorKind::from_status(StatusCode::from_u16(status).unwrap(), b"");
        assert!(matches!(kind(404), HsErrorKind::NotFound(_)));
        assert!(matches!(kind(429), HsErrorKind::RateLimited(_)));
        assert!(matches!(kind(503), HsErrorKind::ServerError(503, _)));
        assert!(matches!(kind(400), HsErrorKind::Rejected(400, _)));
        assert!(matches!(kind(302), HsErrorKind::Rejected(302, _)));
    }

    /// Homeserver text is capped when the kind is built, so a huge body or message
    /// never reaches a log line.
    #[test]
    fn homeserver_text_is_capped() {
        let huge = "x".repeat(MAX_ERROR_BODY * 4);

        let status = HsErrorKind::from_status(StatusCode::BAD_GATEWAY, huge.as_bytes());
        let sdk_status = HsErrorKind::from_sdk(pubky::Error::from(RequestError::Server {
            status: StatusCode::BAD_GATEWAY,
            message: huge.clone(),
        }));
        let sdk = HsErrorKind::from_sdk(pubky::Error::from(RequestError::Validation {
            message: huge.clone(),
        }));
        let transport = HsErrorKind::transport(&huge);
        let malformed = HsErrorKind::malformed(&huge);

        for kind in [status, sdk_status, sdk, transport, malformed] {
            let message = kind.to_string();
            assert!(message.ends_with("… (truncated)"), "{message}");
            assert!(message.len() < MAX_ERROR_BODY + 64, "{}", message.len());
        }
    }

    /// A request that timed out is its own kind, so a log tells it from a reset connection.
    #[tokio::test]
    async fn timed_out_requests_are_classified_as_timeouts() {
        let kind = HsErrorKind::from_sdk(pubky::Error::from(RequestError::Transport(
            timeout_error().await,
        )));

        assert!(matches!(kind, HsErrorKind::Timeout), "{kind:?}");
    }

    /// A failure to reach the homeserver or read its answer is a transport failure.
    #[tokio::test]
    async fn transport_failures_are_classified() {
        let error = pubky::Error::from(RequestError::Transport(transport_error().await));
        let kind = HsErrorKind::from_sdk(error);
        assert!(matches!(kind, HsErrorKind::Transport(_)), "{kind:?}");
    }
}
