use std::fmt::Display;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::db::{kv::RedisError, GraphError, PubkyClientError};
use crate::models::error::ModelError;

#[derive(Error, Debug, Clone, Serialize, Deserialize)]
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

    /// The Pubky client could not resolve the pubky
    #[error("PubkyClientError: {0}")]
    PubkyClientError(#[from] PubkyClientError),

    /// A homeserver's /events-stream keeps returning 429 Too Many Requests
    /// even after all internal backoff retries were exhausted.
    #[error("HS /events-stream rate limit exhausted (429 after all backoff retries)")]
    HsEventsStreamRateLimitExhausted,

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
            ModelError::Generic(message) => EventProcessorError::Generic(message),
        }
    }
}

impl From<pubky::Error> for EventProcessorError {
    fn from(e: pubky::Error) -> Self {
        EventProcessorError::PubkyClientError(PubkyClientError::from(e))
    }
}

impl From<std::io::Error> for EventProcessorError {
    fn from(e: std::io::Error) -> Self {
        EventProcessorError::InternalError(e.to_string())
    }
}

impl From<RedisError> for EventProcessorError {
    fn from(e: RedisError) -> Self {
        let should_not_retry_now = e.should_not_retry_now();
        EventProcessorError::IndexOperationFailed(should_not_retry_now, e.to_string())
    }
}

impl From<GraphError> for EventProcessorError {
    fn from(e: GraphError) -> Self {
        let should_not_retry_now = e.should_not_retry_now();
        EventProcessorError::GraphQueryFailed(should_not_retry_now, e.to_string())
    }
}

impl EventProcessorError {
    pub fn missing_dependencies(dependency_uris: Vec<String>) -> Self {
        Self::MissingDependency {
            dependency: dependency_uris,
        }
    }

    pub fn client_error(message: String) -> Self {
        Self::PubkyClientError(PubkyClientError::RequestFailed { message })
    }

    pub fn client_error_404(message: String) -> Self {
        Self::PubkyClientError(PubkyClientError::NotFound404 { message })
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
    #[allow(clippy::match_like_matches_macro)]
    pub fn should_not_retry_now(&self) -> bool {
        match self {
            Self::GraphQueryFailed(true, _) => true,
            Self::IndexOperationFailed(true, _) => true,
            Self::HsEventsStreamRateLimitExhausted => true,

            _ => false,
        }
    }

    /// Returns whether this error is transient and worth queuing for retry.
    ///
    /// Default is **retryable**: when in doubt we enqueue rather than drop, since
    /// `max_retries` bounds the waste on a misclassified deterministic error,
    /// while silently dropping a misclassified transient error loses data outright.
    /// Only variants we know to be deterministic at conversion time opt out.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::PubkyClientError(err) => err.is_retryable(),
            Self::InvalidEventLine(_) => false,
            Self::SkipIndexing => false,
            Self::UserIdMismatch { .. } => false,
            Self::HsEventsStreamRateLimitExhausted => false,
            _ => true,
        }
    }

    /// Returns whether this error is a 404 from the Pubky client.
    pub fn is_not_found(&self) -> bool {
        matches!(
            self,
            Self::PubkyClientError(PubkyClientError::NotFound404 { .. })
        )
    }

    pub fn is_too_many_requests(&self) -> bool {
        matches!(
            self,
            Self::PubkyClientError(PubkyClientError::TooManyRequests429 { .. })
        )
    }

    /// Returns whether this error is a missing dependency
    pub fn is_missing_dependency(&self) -> bool {
        matches!(self, Self::MissingDependency { .. })
    }
}
