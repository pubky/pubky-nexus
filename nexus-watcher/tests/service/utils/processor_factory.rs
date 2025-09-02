use std::collections::HashMap;
use nexus_common::types::DynError;
use nexus_watcher::{TEventProcessor, TEventProcessorFactory};
use crate::service::utils::processor::MockEventProcessor;

/// Store processors as concrete MockEventProcessor instances.
/// This allows access to the fields for testing purposes.
pub struct MockEventProcessorFactory {
    // TODO: In some point, we could use Box<dyn TEventProcessor> instead of MockEventProcessor
    pub event_processors: HashMap<String, MockEventProcessor>,
}

impl MockEventProcessorFactory {
    pub fn new(event_processors: HashMap<String, MockEventProcessor>) -> Self {
        Self { event_processors }
    }
}

#[async_trait::async_trait]
impl TEventProcessorFactory for MockEventProcessorFactory {
    async fn build(&self, homeserver_id: String) -> Result<Box<dyn TEventProcessor>, DynError> {
        let processor = self.event_processors
            .get(&homeserver_id)
            .ok_or_else(|| DynError::from(format!("no processor found for homeserver_id: {}", homeserver_id)))?;
        Ok(Box::new(MockEventProcessor::new(
            processor.processor_status.clone(),
            processor.timeout.clone(),
            processor.homeserver_id.clone(),
        )))
    }
}
