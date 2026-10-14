use std::fmt::Display;

use pubky::errors::RequestError;
use pubky_app_specs::PubkyId;
use reqwest::StatusCode;

use super::fetch::{format_error_body, MAX_ERROR_BODY};

/// A failed [`HomeserverClient`](super::HomeserverClient) call.
#[derive(Debug, Clone, thiserror::Error)]
#[error("homeserver {hs} {op:?}: {kind}")]
pub struct HsError {
    /// ID of the homeserver the call was charged to. Not a [`PubkyId`], which also holds
    /// the decoded key and would make every error several times larger.
    pub hs: String,
    pub op: HsOp,
    pub kind: HsErrorKind,
}

impl HsError {
    pub fn new(hs: &PubkyId, op: HsOp, kind: HsErrorKind) -> Self {
        Self {
            hs: hs.to_string(),
            op,
            kind,
        }
    }
}

/// The [`HomeserverClient`](super::HomeserverClient) operation a call made.
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
    /// A response, or a request built from homeserver data, that cannot be used as is.
    #[error("malformed: {0}")]
    Malformed(String),
    /// The homeserver's health says to skip it; nothing was sent.
    #[error("circuit open")]
    CircuitOpen,
    #[error("homeserver is blacklisted")]
    Quarantined,
    /// The caller's deadline passed before anything was sent.
    #[error("budget exhausted")]
    BudgetExhausted,
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

    /// Classifies an SDK error the way `PubkyClientError::from(pubky::Error)` does.
    pub fn from_sdk(error: pubky::Error) -> Self {
        match error {
            pubky::Error::Request(RequestError::Server { status, message }) => {
                Self::from_status(status, message.as_bytes())
            }
            pubky::Error::Request(RequestError::Transport(e)) => Self::from_reqwest(&e),
            pubky::Error::Pkarr(e) => Self::transport(e),
            other => Self::malformed(other),
        }
    }
}

/// `error`'s message, cut to [`MAX_ERROR_BODY`] bytes.
fn capped(error: impl Display) -> String {
    format_error_body(error.to_string().as_bytes(), MAX_ERROR_BODY)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let transport = HsErrorKind::transport(&huge);
        let malformed = HsErrorKind::malformed(&huge);

        for kind in [status, transport, malformed] {
            let message = kind.to_string();
            assert!(message.ends_with("… (truncated)"), "{message}");
            assert!(message.len() < MAX_ERROR_BODY + 64, "{}", message.len());
        }
    }
}
