use std::fmt::Display;
use std::sync::Arc;

use nexus_common::db::kv::RedisError;
use nexus_common::db::{GraphError, PubkyClientError};
use nexus_common::models::error::ModelError;
use thiserror::Error;

use crate::homeserver::{HsError, HsErrorKind, HsOp};

#[derive(Error, Debug, Clone)]
pub enum EventProcessorError {
    /// Failed to execute query in the graph database
    #[error("GraphQueryFailed (should_not_retry_now: {0}): {1}")]
    GraphQueryFailed(bool, String),

    /// The event could not be indexed due to missing graph dependencies
    #[error("MissingDependency: Could not be indexed")]
    MissingDependency { dependency: Vec<String> },

    /// Failed to complete indexing due to a Redis operation error
    #[error("IndexOperationFailed (should_not_retry_now: {0}): Indexing incomplete due to Redis error: {1}")]
    IndexOperationFailed(bool, String),

    /// The event appears to be unindexed. Verify the event in the retry queue
    #[error("SkipIndexing: The PUT event appears to be unindexed, so we cannot delete an object that doesn't exist")]
    SkipIndexing,

    /// The event could not be parsed from a line
    #[error("InvalidEventLine: {0}")]
    InvalidEventLine(String),

    #[error("HS returned an event for different user than expected: hs_id={hs_id}, expected={expected_user_id}, received={event_user_id}")]
    UserIdMismatch {
        hs_id: String,
        expected_user_id: String,
        event_user_id: String,
    },

    /// An external HS returned a per-user event cursor at or below the cursor
    /// floor: the user's stored cursor, or an earlier event in the same batch.
    #[error("HS returned an out-of-order event cursor: hs_id={hs_id}, user_id={user_id}, cursor={cursor}, cursor_floor={cursor_floor}")]
    EventCursorOutOfOrder {
        hs_id: String,
        user_id: String,
        cursor: u64,
        cursor_floor: u64,
    },

    /// The event payload deserialized but failed `pubky-social-specs` v0 validation
    /// (e.g. unknown post kind, malformed Collection envelope, oversized field).
    /// Non-retryable: re-running the same payload will produce the same error.
    #[error("SpecValidation: {0}")]
    SpecValidation(String),

    /// A keyset scan could not advance: a deterministic data anomaly, so retrying reproduces it.
    #[error("DataAnomaly: {0}")]
    DataAnomaly(String),

    /// The event is valid but targets a resource kind this handler does not index (e.g. a tag on
    /// something that is neither a Post nor a User, or a bookmark of something that is not a
    /// Post). Non-retryable: the target kind will not change.
    #[error("UnsupportedTarget: {0}")]
    UnsupportedTarget(String),

    /// An edit names a different reply parent or repost embed than the post holds. Both are set
    /// when the post is created, so the edit is rejected whole. Non-retryable: the same payload
    /// is rejected again.
    #[error("PostParentChanged: {0}")]
    PostParentChanged(String),

    /// Fetch exceeded size cap (non-retryable).
    #[error("FetchSizeExceeded: {0} bytes (limit: {1} bytes)")]
    FetchSizeExceeded(u64, u64),

    /// The Pubky client could not resolve the pubky
    #[error("PubkyClientError: {0}")]
    PubkyClientError(Arc<PubkyClientError>),

    /// A homeserver's /events-stream keeps returning 429 Too Many Requests
    /// even after all internal backoff retries were exhausted.
    #[error("HS /events-stream rate limit exhausted (429 after all backoff retries)")]
    HsEventsStreamRateLimitExhausted,

    /// Transport to an external homeserver's /events-stream failed.
    #[error("HsEventsStreamTransportFailed: {0}")]
    HsEventsStreamTransportFailed(String),
    /// An external homeserver's /events-stream answered with a 5xx.
    #[error("HsEventsStreamServerError: {0}")]
    HsEventsStreamServerError(String),

    /// The HS is blacklisted and must not be indexed.
    #[error("HsBlacklisted: {hs_id}")]
    HsBlacklisted { hs_id: String },

    /// A homeserver request that could not be built ([`HsErrorKind::Malformed`]). Not
    /// retried: a fetch's event is dropped, and an event-stream request's user is skipped
    /// for the run, cursor kept. The run goes on.
    #[error("{0}")]
    Homeserver(HsError),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("StaticSaveFailed: {0}")]
    StaticSaveFailed(String),

    /// Catch-all for miscellaneous errors in the processor layer
    #[error("Generic error: {0}")]
    Generic(String),
}

impl From<ModelError> for EventProcessorError {
    fn from(e: ModelError) -> Self {
        match e {
            ModelError::GraphOperationFailed(source) => source.into(),
            ModelError::KvOperationFailed(source) => {
                let should_not_retry_now = source.should_not_retry_now();
                EventProcessorError::IndexOperationFailed(should_not_retry_now, source.to_string())
            }
            ModelError::HsBlacklisted { hs_id } => EventProcessorError::HsBlacklisted { hs_id },
            ModelError::InvalidInput(message) => EventProcessorError::SpecValidation(message),
            ModelError::MissingDependency(uri) => {
                EventProcessorError::missing_dependencies(vec![uri])
            }
            ModelError::Generic(message) => EventProcessorError::Generic(message),
        }
    }
}

impl From<PubkyClientError> for EventProcessorError {
    fn from(e: PubkyClientError) -> Self {
        EventProcessorError::PubkyClientError(Arc::new(e))
    }
}

impl From<pubky::Error> for EventProcessorError {
    fn from(e: pubky::Error) -> Self {
        PubkyClientError::from(e).into()
    }
}

/// Maps a homeserver call's failure onto the variants the processors classify.
///
/// The `/events-stream` request is the homeserver itself, so a 5xx, a transport
/// failure or an exhausted 429 there fails the whole run. On any other request
/// (a file a user can point at any host) they stay per-event.
impl From<HsError> for EventProcessorError {
    fn from(error: HsError) -> Self {
        let on_stream = error.op == HsOp::UserEvents;
        let message = error.to_string();
        match error.kind {
            HsErrorKind::TooLarge { got, max } => Self::FetchSizeExceeded(got, max),
            HsErrorKind::RateLimited(_) if on_stream => Self::HsEventsStreamRateLimitExhausted,
            HsErrorKind::ServerError(..) if on_stream => Self::HsEventsStreamServerError(message),
            HsErrorKind::Transport(_) | HsErrorKind::Timeout if on_stream => {
                Self::HsEventsStreamTransportFailed(message)
            }
            HsErrorKind::NotFound(_) => PubkyClientError::NotFound404 { message }.into(),
            HsErrorKind::RateLimited(_) => PubkyClientError::TooManyRequests429 { message }.into(),
            HsErrorKind::ServerError(..) => PubkyClientError::ServerError5xx { message }.into(),
            HsErrorKind::Rejected(..)
            | HsErrorKind::Transport(_)
            | HsErrorKind::Timeout
            | HsErrorKind::Sdk(_) => PubkyClientError::RequestFailed { message }.into(),
            HsErrorKind::Malformed(_) => Self::Homeserver(error),
        }
    }
}

impl From<std::io::Error> for EventProcessorError {
    fn from(e: std::io::Error) -> Self {
        EventProcessorError::InternalError(e.to_string())
    }
}

impl From<RedisError> for EventProcessorError {
    fn from(e: RedisError) -> Self {
        EventProcessorError::IndexOperationFailed(e.should_not_retry_now(), e.to_string())
    }
}

impl From<GraphError> for EventProcessorError {
    fn from(e: GraphError) -> Self {
        match e {
            GraphError::KeysetScanStalled { .. } => EventProcessorError::DataAnomaly(e.to_string()),
            _ => EventProcessorError::GraphQueryFailed(e.should_not_retry_now(), e.to_string()),
        }
    }
}

impl EventProcessorError {
    pub fn missing_dependencies(dependency_uris: Vec<String>) -> Self {
        Self::MissingDependency {
            dependency: dependency_uris,
        }
    }

    pub fn client_error_404(message: String) -> Self {
        PubkyClientError::NotFound404 { message }.into()
    }

    pub fn static_save_failed(source: impl Display) -> Self {
        Self::StaticSaveFailed(source.to_string())
    }

    pub fn generic(source: impl Display) -> Self {
        Self::Generic(source.to_string())
    }

    pub fn internal_error(source: impl Display) -> Self {
        Self::InternalError(source.to_string())
    }

    /// Returns whether processing should stop and this error should be propagated
    /// to the caller instead of being handled as an isolated event.
    ///
    /// This method does not schedule a retry or backoff itself. Callers decide the
    /// consequence:
    ///
    /// - Event-processing loops abort the current batch or HS run.
    /// - In the external key-based runner, a propagated error produces a non-`Ok`
    ///   run status and therefore triggers per-HS backoff.
    /// - The retry processor reschedules the event without consuming its retry
    ///   allowance, then stops its current batch.
    ///
    /// `false` does not necessarily mean the error will be retried; the caller may
    /// skip it, enqueue it, or continue processing.
    pub fn should_not_retry_now(&self) -> bool {
        matches!(
            self,
            Self::GraphQueryFailed(true, _)
                | Self::IndexOperationFailed(true, _)
                | Self::HsEventsStreamRateLimitExhausted
                | Self::HsEventsStreamTransportFailed(_)
                // Only the /events-stream request maps a 5xx to this variant. A 5xx
                // on any other fetch (a file a user can point at any host) stays a
                // per-event error, or one such file would stall the whole run.
                | Self::HsEventsStreamServerError(_)

                // For a key-based runner, this indicates the external HS either
                // has a temporary glitch or is malicious. Including it here to
                // abort this run and let the runner apply per-HS backoff.
                | Self::EventCursorOutOfOrder { .. }
        )
    }

    /// Returns whether this error is a 404 from the Pubky client.
    pub fn is_not_found(&self) -> bool {
        matches!(
            self,
            Self::PubkyClientError(e) if matches!(e.as_ref(), PubkyClientError::NotFound404 { .. })
        )
    }

    pub fn is_missing_dependency(&self) -> bool {
        matches!(self, Self::MissingDependency { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::retry::RetryScheduler;

    #[test]
    fn event_stream_5xx_is_not_retry_now() {
        let err = EventProcessorError::HsEventsStreamServerError("status 530".to_string());
        assert!(
            err.should_not_retry_now(),
            "an /events-stream 5xx must abort the run so the per-HS backoff applies"
        );
    }

    #[test]
    fn client_5xx_stays_per_event() {
        let err = EventProcessorError::from(PubkyClientError::ServerError5xx {
            message: "File request failed with status 500".to_string(),
        });
        assert!(
            !err.should_not_retry_now(),
            "a 5xx outside the event stream is per-event and must not abort the run"
        );
    }

    fn hs_error(op: HsOp, kind: HsErrorKind) -> EventProcessorError {
        let hs = pubky_social_specs::PubkyId::try_from(
            "1hb71xx9km3f4pw5izsy1gn19ff1uuuqonw4mcygzobwkryujoiy",
        )
        .unwrap();
        HsError::new(&hs, op, kind).into()
    }

    /// On the event stream, a 5xx, a transport failure or an exhausted 429 fail the
    /// whole homeserver run, so the runner backs it off.
    #[test]
    fn event_stream_failures_abort_the_run() {
        let server_error = hs_error(HsOp::UserEvents, HsErrorKind::ServerError(530, "".into()));
        let transport = hs_error(
            HsOp::UserEvents,
            HsErrorKind::transport("connection refused"),
        );
        let timeout = hs_error(HsOp::UserEvents, HsErrorKind::Timeout);
        let rate_limited = hs_error(HsOp::UserEvents, HsErrorKind::RateLimited("".into()));

        assert!(matches!(
            server_error,
            EventProcessorError::HsEventsStreamServerError(_)
        ));
        assert!(matches!(
            transport,
            EventProcessorError::HsEventsStreamTransportFailed(_)
        ));
        assert!(matches!(
            timeout,
            EventProcessorError::HsEventsStreamTransportFailed(_)
        ));
        assert!(matches!(
            rate_limited,
            EventProcessorError::HsEventsStreamRateLimitExhausted
        ));
        for err in [server_error, transport, timeout, rate_limited] {
            assert!(err.should_not_retry_now(), "{err}");
        }
    }

    /// The same failures on a resource or blob fetch stay per-event, and a 404, a 403 or
    /// an SDK failure on the event stream stays per-user.
    #[test]
    fn other_failures_stay_per_event() {
        for (op, kind) in [
            (HsOp::GetResource, HsErrorKind::ServerError(500, "".into())),
            (HsOp::GetBlob, HsErrorKind::transport("connection refused")),
            (HsOp::PollEvents, HsErrorKind::RateLimited("".into())),
            (HsOp::UserEvents, HsErrorKind::NotFound("".into())),
            (HsOp::UserEvents, HsErrorKind::Rejected(403, "".into())),
            (
                HsOp::UserEvents,
                HsErrorKind::Sdk("could not resolve".into()),
            ),
        ] {
            let err = hs_error(op, kind);
            assert!(!err.should_not_retry_now(), "{err}");
            assert!(RetryScheduler::should_enqueue_related_event(&err) || err.is_not_found());
        }

        let server_error = hs_error(HsOp::GetResource, HsErrorKind::ServerError(500, "".into()));
        assert!(
            matches!(&server_error, EventProcessorError::PubkyClientError(e) if matches!(e.as_ref(), PubkyClientError::ServerError5xx { .. })),
            "{server_error:?}"
        );
        let rate_limited = hs_error(HsOp::PollEvents, HsErrorKind::RateLimited("".into()));
        assert!(
            matches!(&rate_limited, EventProcessorError::PubkyClientError(e) if matches!(e.as_ref(), PubkyClientError::TooManyRequests429 { .. })),
            "{rate_limited:?}"
        );
        assert!(hs_error(HsOp::UserEvents, HsErrorKind::NotFound("".into())).is_not_found());
    }

    /// A malformed event-stream request fails one user for the run: nothing is enqueued, the
    /// run goes on, and the user's kept cursor has the next run ask again.
    #[test]
    fn malformed_stream_request_skips_one_user_for_the_run() {
        let err = hs_error(HsOp::UserEvents, HsErrorKind::malformed("bad url"));

        assert!(matches!(err, EventProcessorError::Homeserver(_)), "{err:?}");
        assert!(!err.should_not_retry_now());
        assert!(!RetryScheduler::should_enqueue_related_event(&err));
    }

    /// A 4xx other than 404/429, or a timeout off the event stream, is a retried
    /// `RequestFailed`, not a permanent failure.
    #[test]
    fn rejected_and_timed_out_fetches_are_retried() {
        for (op, kind) in [
            (HsOp::GetResource, HsErrorKind::Rejected(403, "".into())),
            (HsOp::GetBlob, HsErrorKind::Timeout),
        ] {
            let err = hs_error(op, kind);
            assert!(
                matches!(&err, EventProcessorError::PubkyClientError(e) if matches!(e.as_ref(), PubkyClientError::RequestFailed { .. })),
                "{err:?}"
            );
            assert!(
                RetryScheduler::should_enqueue_related_event(&err),
                "{err:?}"
            );
            assert!(!err.should_not_retry_now(), "{err:?}");
        }
    }

    /// An oversized body is a permanent size failure, not retried.
    #[test]
    fn too_large_is_a_size_failure() {
        let err = hs_error(HsOp::GetBlob, HsErrorKind::TooLarge { got: 10, max: 5 });

        assert!(matches!(err, EventProcessorError::FetchSizeExceeded(10, 5)));
        assert!(!RetryScheduler::should_enqueue_related_event(&err));
    }

    /// The message names the homeserver and carries the homeserver's answer.
    #[test]
    fn message_names_the_homeserver_and_the_status() {
        let err = hs_error(
            HsOp::PollEvents,
            HsErrorKind::ServerError(502, "bad gateway".into()),
        );

        let message = err.to_string();
        assert!(
            message.contains("1hb71xx9km3f4pw5izsy1gn19ff1uuuqonw4mcygzobwkryujoiy"),
            "{message}"
        );
        assert!(message.contains("HTTP 502: bad gateway"), "{message}");
    }

    #[test]
    fn hs_not_found_404_stays_retryable_per_user() {
        let err = EventProcessorError::from(PubkyClientError::NotFound404 {
            message: "Not found".to_string(),
        });
        assert!(
            !err.should_not_retry_now(),
            "a 404 is per-user and must not abort the run"
        );
    }
}
