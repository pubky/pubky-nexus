use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::models::error::ModelError;

#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum EventProcessorError {
    /// Failed to execute query in the graph database
    #[error("GraphQueryFailed: {message}")]
    GraphQueryFailed { message: String },
    /// The event could not be indexed due to missing graph dependencies
    #[error("MissingDependency: Could not be indexed")]
    MissingDependency { dependency: Vec<String> },
    /// Failed to complete indexing due to a Redis write error
    #[error("IndexWriteFailed: Indexing incomplete due to Redis error - {message}")]
    IndexWriteFailed { message: String },
    /// The event appears to be unindexed. Verify the event in the retry queue
    #[error("SkipIndexing: The PUT event appears to be unindexed, so we cannot delete an object that doesn't exist")]
    SkipIndexing,
    /// The event could not be parsed from a line
    #[error("InvalidEventLine: {message}")]
    InvalidEventLine { message: String },
    /// The Pubky client could not resolve the pubky
    #[error("PubkyClientError: {0}")]
    PubkyClientError(#[from] crate::db::PubkyClientError),
    #[error("Internal error: {message}")]
    InternalError { message: String },
    #[error("Failed to store event: {message}")]
    FailedToStoreEvent { message: String },
    #[error("Failed to save static resource {message}")]
    FailedToSaveStatic { message: String },
    /// Catch-all for miscellaneous errors in the processor layer
    #[error("Other error: {message}")]
    Other { message: String },
}

impl From<ModelError> for EventProcessorError {
    fn from(e: ModelError) -> Self {
        match e {
            ModelError::GraphOperationFailed { message } => {
                EventProcessorError::GraphQueryFailed { message }
            }
            ModelError::KvOperationFailed { message } => {
                EventProcessorError::IndexWriteFailed { message }
            }
            ModelError::FileOperationFailed { message } => {
                EventProcessorError::InternalError { message }
            }
            ModelError::Other { message } => EventProcessorError::Other { message },
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
        EventProcessorError::InternalError {
            message: e.to_string(),
        }
    }
}

impl EventProcessorError {
    pub fn missing_dependencies(dependency_uris: Vec<String>) -> Self {
        Self::MissingDependency {
            dependency: dependency_uris,
        }
    }

    pub fn client_error(message: String) -> Self {
        crate::db::PubkyClientError::ClientError(message).into()
    }

    pub fn index_write_failed(source: impl std::fmt::Display) -> Self {
        Self::IndexWriteFailed {
            message: source.to_string(),
        }
    }

    pub fn internal_error(source: impl std::fmt::Display) -> Self {
        Self::InternalError {
            message: source.to_string(),
        }
    }

    pub fn failed_to_store_event(source: impl std::fmt::Display) -> Self {
        Self::FailedToStoreEvent {
            message: source.to_string(),
        }
    }

    pub fn failed_to_save_static(source: impl std::fmt::Display) -> Self {
        Self::FailedToSaveStatic {
            message: source.to_string(),
        }
    }

    pub fn graph_query_failed(source: impl std::fmt::Display) -> Self {
        Self::GraphQueryFailed {
            message: source.to_string(),
        }
    }

    pub fn other(source: impl std::fmt::Display) -> Self {
        Self::Other {
            message: source.to_string(),
        }
    }
}
