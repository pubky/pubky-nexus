use serde::{Deserialize, Serialize};
use thiserror::Error;

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
    /// Catch-all for miscellaneous errors in the processor layer
    #[error("{0}")]
    Other(String),
}

// impl From<crate::types::DynError> for EventProcessorError {
//     fn from(e: crate::types::DynError) -> Self {
//         EventProcessorError::Other(e.to_string())
//     }
// }

impl From<crate::db::kv::RedisError> for EventProcessorError {
    fn from(e: crate::db::kv::RedisError) -> Self {
        EventProcessorError::IndexWriteFailed {
            message: e.to_string(),
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
        EventProcessorError::Other(e.to_string())
    }
}

impl From<String> for EventProcessorError {
    fn from(s: String) -> Self {
        EventProcessorError::Other(s)
    }
}

impl From<&str> for EventProcessorError {
    fn from(s: &str) -> Self {
        EventProcessorError::Other(s.to_string())
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
}
