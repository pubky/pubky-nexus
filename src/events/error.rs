use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum EventProcessorError {
    #[error("The user could not be indexed in nexus")]
    UserNotSync,
    #[error("The event could not be indexed because some graph dependency is missing")]
    MissingDependency { dependency: Vec<String> },
    #[error("The event does not exist anymore in the homeserver")]
    ContentNotFound { dependency: String },
    #[error("PubkyClient could not reach/resolve the homeserver")]
    NotResolvedHomeserver,
    #[error("The event could not be parsed from a line")]
    InvalidEvent,
    #[error("")]
    PubkyClientError,
}
