use crate::service::utils::processor::MockEventProcessor;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use nexus_watcher::service::{TEventProcessor, TEventProcessorFactory};
use std::sync::Arc;
use tokio::sync::watch::Receiver;

/// Store processors as concrete MockEventProcessor instances.
/// This allows access to the fields for testing purposes.
pub struct MockEventProcessorFactory {
    /// The event processors to be used by the factory
    pub event_processors: Vec<Arc<MockEventProcessor>>,
    pub shutdown_rx: Receiver<bool>,
}

impl MockEventProcessorFactory {
    /// Creates a new factory instance from the provided event processors
    pub fn new(event_processors: Vec<MockEventProcessor>, shutdown_rx: Receiver<bool>) -> Self {
        let arcs: Vec<Arc<MockEventProcessor>> = event_processors
            .into_iter()
            .map(|mock_event_processor| Arc::new(mock_event_processor))
            .collect();

        Self {
            event_processors: arcs,
            shutdown_rx,
        }
    }
}

#[async_trait::async_trait]
impl TEventProcessorFactory for MockEventProcessorFactory {
    fn shutdown_rx(&self) -> Receiver<bool> {
        self.shutdown_rx.clone()
    }

    fn default_homeserver(&self) -> &str {
        // Use first mock homeserver ID if available, otherwise fallback to mock constant
        self.event_processors
            .first()
            .map(|s| s.homeserver_id.as_str())
            .unwrap_or("8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo")
    }

    /// Returns homeserver IDs with the insert order of the event processors
    async fn homeservers_by_priority(&self) -> Vec<String> {
        let persistedhs_ids = Homeserver::get_all_from_graph()
            .await
            .expect("No Homeserver IDs found in graph");

        let mut hs_ids = vec![];

        // Skip the homeserver IDs that are not part of the factory's event processors
        for mock_event_processor in self.event_processors.iter() {
            let hs_id = mock_event_processor.homeserver_id.to_string().clone();
            if persistedhs_ids.contains(&hs_id) {
                hs_ids.push(hs_id);
            }
        }

        hs_ids
    }

    /// Returns the event processor for the specified homeserver.
    ///
    /// The mock event processor was pre-built and given to the mock factory on initialization, so this returns a reference to it.
    async fn build(&self, homeserver_id: String) -> Result<Arc<dyn TEventProcessor>, DynError> {
        let mock_event_processor = self
            .event_processors
            .iter()
            .find(|p| p.homeserver_id.to_string() == homeserver_id)
            .cloned()
            .ok_or(format!("No MockEventProcessor for HS ID: {homeserver_id}"))?;

        Ok(mock_event_processor)
    }
}
