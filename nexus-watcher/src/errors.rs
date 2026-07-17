use std::fmt::Display;
use std::sync::Arc;

use nexus_common::db::kv::RedisError;
use nexus_common::db::{GraphError, PubkyClientError};
use nexus_common::models::error::ModelError;
use thiserror::Error;

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

    /// A homeserver returned a per-user event cursor lower than an earlier event.
    #[error("HS returned an out-of-order event cursor: hs_id={hs_id}, user_id={user_id}, cursor={cursor}, max_cursor={max_cursor}")]
    EventCursorOutOfOrder {
        hs_id: String,
        user_id: String,
        cursor: u64,
        max_cursor: u64,
    },

    /// The event payload deserialized but failed `pubky-app-specs` validation
    /// (e.g. unknown post kind, malformed Collection envelope, oversized field).
    /// Non-retryable: re-running the same payload will produce the same error.
    #[error("SpecValidation: {0}")]
    SpecValidation(String),

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

    /// The HS is blacklisted and must not be indexed.
    #[error("HsBlacklisted: {hs_id}")]
    HsBlacklisted { hs_id: String },

    #[error("MediaProcessor: {0}")]
    MediaProcessorError(String),

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
            ModelError::GraphOperationFailed(source) => {
                let should_not_retry_now = source.should_not_retry_now();
                EventProcessorError::GraphQueryFailed(should_not_retry_now, source.to_string())
            }
            ModelError::KvOperationFailed(source) => {
                let should_not_retry_now = source.should_not_retry_now();
                EventProcessorError::IndexOperationFailed(should_not_retry_now, source.to_string())
            }
            ModelError::MediaProcessorError(source) => {
                EventProcessorError::MediaProcessorError(source.to_string())
            }
            ModelError::FileOperationFailed(source) => {
                EventProcessorError::InternalError(source.to_string())
            }
            ModelError::HsBlacklisted { hs_id } => EventProcessorError::HsBlacklisted { hs_id },
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
        EventProcessorError::GraphQueryFailed(e.should_not_retry_now(), e.to_string())
    }
}

impl EventProcessorError {
    pub fn missing_dependencies(dependency_uris: Vec<String>) -> Self {
        Self::MissingDependency {
            dependency: dependency_uris,
        }
    }

    pub fn client_error(message: String) -> Self {
        PubkyClientError::RequestFailed { message }.into()
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

    /// Returns whether or not we should refrain from retrying this error right now.
    ///
    /// These are the kinds of errors that are expected to be thrown again
    /// if the event processor caller continues processing other events.
    pub fn should_not_retry_now(&self) -> bool {
        matches!(
            self,
            Self::GraphQueryFailed(true, _)
                | Self::IndexOperationFailed(true, _)
                | Self::HsEventsStreamRateLimitExhausted
        )
    }

    /// Returns whether this error is a 404 from the Pubky client.
    pub fn is_not_found(&self) -> bool {
        matches!(
            self,
            Self::PubkyClientError(e) if matches!(e.as_ref(), PubkyClientError::NotFound404 { .. })
        )
    }

    pub fn is_too_many_requests(&self) -> bool {
        matches!(
            self,
            Self::PubkyClientError(e) if matches!(e.as_ref(), PubkyClientError::TooManyRequests429 { .. })
        )
    }

    pub fn is_missing_dependency(&self) -> bool {
        matches!(self, Self::MissingDependency { .. })
    }
}
