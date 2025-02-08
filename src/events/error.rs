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
    /// The event appear to be unindexed. Verify the event in the retry queue
    #[error("SkipIndexing: The PUT event appears to be unindexed")]
    SkipIndexing,
    /// The event could not be parsed from a line
    #[error("InvalidEventLine: {message}")]
    InvalidEventLine { message: String },
    /// The Pubky client could not resolve the pubky
    #[error("PubkyClientError: {message}")]
    PubkyClientError { message: String },
    // #[error("The event does not exist anymore in the homeserver")]
    // ContentNotFound { dependency: String },
    // #[error("PubkyClient could not reach/resolve the homeserver")]
    // NotResolvedHomeserver,
}
