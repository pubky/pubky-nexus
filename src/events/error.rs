use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum EventProcessorError {
    #[error("The user could not be indexed in nexus")]
    UserNotSync,
    #[error("The event could not be indexed due to missing graph dependenciesz")]
    MissingDependency { dependency: Vec<String> },
    #[error("The event appear to be unindexed. Verify the event in the retry queue")]
    SkipIndexing,
    #[error("The event does not exist anymore in the homeserver")]
    ContentNotFound { dependency: String },
    #[error("PubkyClient could not reach/resolve the homeserver")]
    NotResolvedHomeserver,
    #[error("The event could not be parsed from a line")]
    InvalidEvent,
    #[error("")]
    PubkyClientError,
}
