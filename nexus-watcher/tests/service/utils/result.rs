use nexus_common::types::DynError;

/// Result of a homeserver event processing
pub enum MockEventProcessorResult {
    Success(String),
    Error(DynError),
    Panic(),
}

impl Clone for MockEventProcessorResult {
    /// Useful native function for testing environment
    fn clone(&self) -> Self {
        match self {
            MockEventProcessorResult::Success(msg) => {
                MockEventProcessorResult::Success(msg.clone())
            }
            MockEventProcessorResult::Error(err) => {
                MockEventProcessorResult::Error(err.to_string().into())
            }
            MockEventProcessorResult::Panic() => MockEventProcessorResult::Panic(),
        }
    }
}
