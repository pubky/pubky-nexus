use crate::service::utils::processor::MockEventProcessor;
use nexus_common::types::DynError;
use nexus_watcher::events::{TEventProcessor, TEventProcessorFactory};
use nexus_watcher::service::PROCESSING_TIMEOUT_SECS;
use std::sync::Arc;
use std::{collections::HashMap, time::Duration};

/// Store processors as concrete MockEventProcessor instances.
/// This allows access to the fields for testing purposes.
pub struct MockEventProcessorFactory {
    pub event_processors: HashMap<String, Arc<MockEventProcessor>>,
    pub timeout: Option<Duration>,
}

impl MockEventProcessorFactory {
    /// Creates a new factory instance from the provided event processors
    pub fn new(
        event_processors: HashMap<String, MockEventProcessor>,
        timeout: Option<Duration>,
    ) -> Self {
        let arcs: HashMap<String, Arc<MockEventProcessor>> = event_processors
            .into_iter()
            .map(|(k, v)| (k, Arc::new(v)))
            .collect();

        Self {
            event_processors: arcs,
            timeout,
        }
    }
}

#[async_trait::async_trait]
impl TEventProcessorFactory for MockEventProcessorFactory {
    /// Returns the timeout for the event processor
    fn timeout(&self) -> Duration {
        match self.timeout {
            Some(timeout) => timeout,
            None => Duration::from_secs(PROCESSING_TIMEOUT_SECS),
        }
    }

    /// Returns the event processor for the specified homeserver.
    ///
    /// The mock event processor was pre-built and given to the mock factory on initialization, so this returns a reference to it.
    async fn build(&self, homeserver_id: String) -> Result<Arc<dyn TEventProcessor>, DynError> {
        let mock_event_processor = self
            .event_processors
            .get(&homeserver_id)
            .cloned()
            .ok_or(format!("No MockEventProcessor for HS ID: {homeserver_id}"))?;

        Ok(mock_event_processor)
    }
}
