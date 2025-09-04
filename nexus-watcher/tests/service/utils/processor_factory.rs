use crate::service::utils::processor::MockEventProcessor;
use nexus_common::types::DynError;
use nexus_watcher::events::{TEventProcessor, TEventProcessorFactory};
use std::collections::HashMap;

/// Store processors as concrete MockEventProcessor instances.
/// This allows access to the fields for testing purposes.
pub struct MockEventProcessorFactory {
    // TODO: In some point, we could use Box<dyn TEventProcessor> instead of MockEventProcessor
    pub event_processors: HashMap<String, MockEventProcessor>,
}

impl MockEventProcessorFactory {
    /// Creates a new factory instance from the provided event processors
    pub fn new(event_processors: HashMap<String, MockEventProcessor>) -> Self {
        Self { event_processors }
    }
}

#[async_trait::async_trait]
impl TEventProcessorFactory for MockEventProcessorFactory {
    /// Creates and returns a new event processor instance for the specified homeserver
    /// The ownership of the event processor is transferred to the caller
    async fn build(&self, homeserver_id: String) -> Result<Box<dyn TEventProcessor>, DynError> {
        let processor = self.event_processors.get(&homeserver_id).ok_or_else(|| {
            DynError::from(format!(
                "no processor found for homeserver_id: {}",
                homeserver_id
            ))
        })?;
        // Create a new event processor instance with the specified homeserver
        Ok(Box::new(MockEventProcessor::new(
            processor.processor_status.clone(),
            processor.timeout.clone(),
            processor.homeserver_id.clone(),
        )))
    }
}
