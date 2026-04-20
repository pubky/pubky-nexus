use std::fmt::Display;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::db::{kv::RedisError, GraphError, PubkyClientError, PubkyClientErrorKind};
use crate::models::error::ModelError;

#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum EventProcessorError {
    /// Failed to execute query in the graph database
    #[error("GraphQueryFailed (is_infrastructure_err: {0}): {1}")]
    GraphQueryFailed(bool, String),

    /// The event could not be indexed due to missing graph dependencies
    #[error("MissingDependency: Could not be indexed")]
    MissingDependency { dependency: Vec<String> },

    /// Failed to complete indexing due to a Redis operation error
    #[error("IndexOperationFailed (is_infrastructure_err: {0}): Indexing incomplete due to Redis error: {1}")]
    IndexOperationFailed(bool, String),

    /// The event appears to be unindexed. Verify the event in the retry queue
    #[error("SkipIndexing: The PUT event appears to be unindexed, so we cannot delete an object that doesn't exist")]
    SkipIndexing,

    /// The event could not be parsed from a line
    #[error("InvalidEventLine: {0}")]
    InvalidEventLine(String),

    /// The Pubky client could not resolve the pubky
    #[error("PubkyClientError: {0}")]
    PubkyClientError(#[from] PubkyClientError),

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
                let is_infrastructure_err = source.is_infrastructure_err();
                EventProcessorError::GraphQueryFailed(is_infrastructure_err, source.to_string())
            }
            ModelError::KvOperationFailed(source) => {
                let is_infrastructure_err = source.is_infrastructure_err();
                EventProcessorError::IndexOperationFailed(is_infrastructure_err, source.to_string())
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
        let kind = PubkyClientErrorKind::from_pubky_error(e);
        EventProcessorError::PubkyClientError(PubkyClientError::ClientError(kind))
    }
}

impl From<std::io::Error> for EventProcessorError {
    fn from(e: std::io::Error) -> Self {
        EventProcessorError::InternalError(e.to_string())
    }
}

impl From<RedisError> for EventProcessorError {
    fn from(e: RedisError) -> Self {
        let is_infrastructure_err = e.is_infrastructure_err();
        EventProcessorError::IndexOperationFailed(is_infrastructure_err, e.to_string())
    }
}

impl From<GraphError> for EventProcessorError {
    fn from(e: GraphError) -> Self {
        let is_infrastructure_err = e.is_infrastructure_err();
        EventProcessorError::GraphQueryFailed(is_infrastructure_err, e.to_string())
    }
}

impl EventProcessorError {
    pub fn missing_dependencies(dependency_uris: Vec<String>) -> Self {
        Self::MissingDependency {
            dependency: dependency_uris,
        }
    }

    pub fn client_error(message: String) -> Self {
        Self::PubkyClientError(PubkyClientError::ClientError(
            PubkyClientErrorKind::RequestFailed { message },
        ))
    }

    pub fn client_error_404(message: String) -> Self {
        Self::PubkyClientError(PubkyClientError::ClientError(
            PubkyClientErrorKind::NotFound404 { message },
        ))
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

    /// Returns whether or not this is an infrastructure error.
    ///
    /// These are the kinds of errors that are expected to be thrown again,
    /// if the event processor caller continues processing other events.
    #[allow(clippy::match_like_matches_macro)]
    pub fn is_infrastructure(&self) -> bool {
        match self {
            Self::GraphQueryFailed(true, _) => true,
            Self::IndexOperationFailed(true, _) => true,
            _ => false,
        }
    }

    /// Returns whether this error is transient and worth retrying.
    /// Non-retryable errors (ParseFailed, AuthenticationFailed, BuildFailed) should be
    /// dead-lettered immediately.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::PubkyClientError(PubkyClientError::ClientError(kind)) => kind.is_retryable(),
            Self::InvalidEventLine(_) => false,
            Self::SkipIndexing => false,
            _ => true,
        }
    }

    /// Returns whether this error is a missing dependency
    pub fn is_missing_dependency(&self) -> bool {
        matches!(self, Self::MissingDependency { .. })
    }

    /// Returns whether this error indicates a 404 (content definitively gone)
    pub fn is_404(&self) -> bool {
        matches!(
            self,
            Self::PubkyClientError(PubkyClientError::ClientError(kind))
                if kind.is_404()
        )
    }
}
