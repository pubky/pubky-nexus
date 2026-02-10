use nexus_common::models::error::ModelError;
use nexus_common::models::event::EventProcessorError;
use nexus_common::types::DynError;

/// Error type for the watcher service layer (runner, builder, startup).
#[derive(Debug, thiserror::Error)]
pub enum WatcherError {
    #[error("{0}")]
    EventProcessor(#[from] EventProcessorError),
    #[error("{0}")]
    Other(String),
}

impl From<ModelError> for WatcherError {
    fn from(e: ModelError) -> Self {
        WatcherError::Other(e.to_string())
    }
}

//todo: should be removed
impl From<DynError> for WatcherError {
    fn from(e: DynError) -> Self {
        WatcherError::Other(e.to_string())
    }
}

impl From<String> for WatcherError {
    fn from(s: String) -> Self {
        WatcherError::Other(s)
    }
}

impl From<&str> for WatcherError {
    fn from(s: &str) -> Self {
        WatcherError::Other(s.to_string())
    }
}
