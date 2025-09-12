mod processor;
mod processor_factory;
mod result;
mod setup;

pub use processor::{create_mock_event_processors, MockEventProcessor, create_random_homeservers_and_persist};
pub use processor_factory::MockEventProcessorFactory;
pub use setup::{setup, HS_IDS};

pub use result::MockEventProcessorResult;

/// Create a success result type
pub fn success_result(message: &str) -> MockEventProcessorResult {
    MockEventProcessorResult::Success(message.to_string())
}

/// Create an error result type
pub fn error_result(message: &str) -> MockEventProcessorResult {
    MockEventProcessorResult::Error(message.to_string().into())
}

/// Create a panic result type
pub fn _panic_result() -> MockEventProcessorResult {
    MockEventProcessorResult::Panic()
}
