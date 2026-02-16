use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    db::{kv::RedisError, GraphError},
    models::error::ModelError,
};

#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum EventProcessorError {
    /// Failed to execute query in the graph database
    #[error("GraphQueryFailed: {0}")]
    GraphQueryFailed(String),
    /// The event could not be indexed due to missing graph dependencies
    #[error("MissingDependency: Could not be indexed")]
    MissingDependency { dependency: Vec<String> },
    /// Failed to complete indexing due to a Redis operation error
    #[error("IndexOperationFailed: Indexing incomplete due to Redis error - {0}")]
    IndexOperationFailed(String),
    /// The event appears to be unindexed. Verify the event in the retry queue
    #[error("SkipIndexing: The PUT event appears to be unindexed, so we cannot delete an object that doesn't exist")]
    SkipIndexing,
    /// The event could not be parsed from a line
    #[error("InvalidEventLine: {0}")]
    InvalidEventLine(String),
    /// The Pubky client could not resolve the pubky
    #[error("PubkyClientError: {0}")]
    PubkyClientError(#[from] crate::db::PubkyClientError),
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
            ModelError::GraphOperationFailed { source } => {
                EventProcessorError::GraphQueryFailed(source.to_string())
            }
            ModelError::KvOperationFailed { source } => {
                EventProcessorError::IndexOperationFailed(source.to_string())
            }
            ModelError::MediaProcessorError { source } => {
                EventProcessorError::MediaProcessorError(source.to_string())
            }
            ModelError::FileOperationFailed { source } => {
                EventProcessorError::InternalError(source.to_string())
            }
            ModelError::Generic(message) => EventProcessorError::Generic(message),
        }
    }
}

impl From<pubky::Error> for EventProcessorError {
    fn from(e: pubky::Error) -> Self {
        EventProcessorError::client_error(e.to_string())
    }
}

impl From<std::io::Error> for EventProcessorError {
    fn from(e: std::io::Error) -> Self {
        EventProcessorError::InternalError(e.to_string())
    }
}

impl From<RedisError> for EventProcessorError {
    fn from(e: RedisError) -> Self {
        EventProcessorError::IndexOperationFailed(e.to_string())
    }
}

impl From<GraphError> for EventProcessorError {
    fn from(e: GraphError) -> Self {
        EventProcessorError::GraphQueryFailed(e.to_string())
    }
}

impl EventProcessorError {
    pub fn missing_dependencies(dependency_uris: Vec<String>) -> Self {
        Self::MissingDependency {
            dependency: dependency_uris,
        }
    }

    pub fn client_error(message: String) -> Self {
        Self::PubkyClientError(crate::db::PubkyClientError::ClientError(message))
    }

    pub fn index_operation_failed(source: impl std::fmt::Display) -> Self {
        Self::IndexOperationFailed(source.to_string())
    }

    pub fn static_save_failed(source: impl std::fmt::Display) -> Self {
        Self::StaticSaveFailed(source.to_string())
    }

    pub fn graph_query_failed(source: impl std::fmt::Display) -> Self {
        Self::GraphQueryFailed(source.to_string())
    }

    pub fn generic(source: impl std::fmt::Display) -> Self {
        Self::Generic(source.to_string())
    }
}
