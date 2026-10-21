use std::fmt::Display;

use pubky::errors::RequestError;
use pubky_app_specs::PubkyId;
use reqwest::StatusCode;

use super::fetch::{format_error_body, MAX_ERROR_BODY};

/// A failed [`HomeserverClient`](super::HomeserverClient) call.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{op:?} {hs}: {kind}")]
pub struct HsError {
    /// ID of the homeserver the call went to, or for a resource or blob, its owner's key.
    /// Not a [`PubkyId`], which also holds the decoded key and would make every error
    /// several times larger.
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
    /// malformed, and permanent.
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
            pubky::Error::Parse(_) | pubky::Error::Authentication(_) | pubky::Error::Build(_) => {
                Self::malformed(error)
            }
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
    use crate::errors::EventProcessorError;
    use crate::events::retry::RetryScheduler;
    use nexus_common::db::PubkyClientError;
    use pubky::errors::{AuthError, PkarrError};

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

        let hs = PubkyId::try_from("1hb71xx9km3f4pw5izsy1gn19ff1uuuqonw4mcygzobwkryujoiy").unwrap();
        let retried: EventProcessorError = HsError::new(&hs, HsOp::GetResource, validation).into();
        let dropped: EventProcessorError = HsError::new(&hs, HsOp::GetResource, parse).into();
        assert!(RetryScheduler::should_enqueue_related_event(&retried));
        assert!(!RetryScheduler::should_enqueue_related_event(&dropped));
        assert!(!dropped.should_not_retry_now());
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

    /// On a fetch, the client's classification and `PubkyClientError::from` decide the same
    /// for every SDK error: whether the run stops, whether the event is retried, and whether
    /// it was a 404. The compiler cannot check the two lists against each other, so this does.
    #[tokio::test]
    async fn sdk_classification_agrees_with_the_shared_converter() {
        let server = |status: u16| {
            pubky::Error::from(RequestError::Server {
                status: StatusCode::from_u16(status).unwrap(),
                message: "nope".into(),
            })
        };
        // Two of each, the SDK's error not being `Clone`.
        let pair = |make: &dyn Fn() -> pubky::Error| (make(), make());
        let mut pairs = vec![
            pair(&|| server(404)),
            pair(&|| server(429)),
            pair(&|| server(503)),
            pair(&|| server(400)),
            pair(&|| {
                pubky::Error::from(RequestError::Validation {
                    message: "bad".into(),
                })
            }),
            pair(&|| {
                pubky::Error::from(RequestError::DecodeJson {
                    message: "bad".into(),
                })
            }),
            pair(&|| pubky::Error::from(PkarrError::InvalidRecord("bad".into()))),
            pair(&|| pubky::Error::from(reqwest::Url::parse("https://").unwrap_err())),
            pair(&|| pubky::Error::from(AuthError::Validation("bad".into()))),
        ];
        pairs.push((
            pubky::Error::from(RequestError::Transport(transport_error().await)),
            pubky::Error::from(RequestError::Transport(transport_error().await)),
        ));

        let hs = PubkyId::try_from("1hb71xx9km3f4pw5izsy1gn19ff1uuuqonw4mcygzobwkryujoiy").unwrap();
        for (via_client, via_converter) in pairs {
            let label = via_converter.to_string();
            let via_client: EventProcessorError =
                HsError::new(&hs, HsOp::GetResource, HsErrorKind::from_sdk(via_client)).into();
            let via_converter = EventProcessorError::from(PubkyClientError::from(via_converter));

            assert_eq!(
                via_client.should_not_retry_now(),
                via_converter.should_not_retry_now(),
                "{label}"
            );
            assert_eq!(
                RetryScheduler::should_enqueue_related_event(&via_client),
                RetryScheduler::should_enqueue_related_event(&via_converter),
                "{label}"
            );
            assert_eq!(
                via_client.is_not_found(),
                via_converter.is_not_found(),
                "{label}"
            );
        }
    }

    /// A transport failure stops the run on the event stream, and is retried on a fetch.
    #[tokio::test]
    async fn transport_failures_stop_the_stream_and_retry_fetches() {
        let error = pubky::Error::from(RequestError::Transport(transport_error().await));
        let kind = HsErrorKind::from_sdk(error);
        assert!(matches!(kind, HsErrorKind::Transport(_)), "{kind:?}");

        let hs = PubkyId::try_from("1hb71xx9km3f4pw5izsy1gn19ff1uuuqonw4mcygzobwkryujoiy").unwrap();
        let on_stream: EventProcessorError =
            HsError::new(&hs, HsOp::UserEvents, kind.clone()).into();
        assert!(on_stream.should_not_retry_now(), "{on_stream:?}");

        let on_blob: EventProcessorError = HsError::new(&hs, HsOp::GetBlob, kind).into();
        assert!(!on_blob.should_not_retry_now(), "{on_blob:?}");
        assert!(RetryScheduler::should_enqueue_related_event(&on_blob));
    }
}
