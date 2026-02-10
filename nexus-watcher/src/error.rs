use nexus_common::models::error::ModelError;
use nexus_common::models::event::EventProcessorError;

/// Error type for the watcher service layer (runner, builder, startup).
#[derive(Debug, thiserror::Error)]
pub enum WatcherError {
    #[error("EventProcessorError: {0}")]
    EventProcessor(#[from] EventProcessorError),
    #[error("Other: {message}")]
    Other { message: String },
}

impl From<ModelError> for WatcherError {
    fn from(e: ModelError) -> Self {
        WatcherError::Other {
            message: e.to_string(),
        }
    }
}

impl WatcherError {
    pub fn other(source: impl std::fmt::Display) -> Self {
        Self::Other {
            message: source.to_string(),
        }
    }
}
