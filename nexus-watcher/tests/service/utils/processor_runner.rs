use crate::service::utils::processor::MockEventProcessor;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use nexus_watcher::service::{TEventProcessor, TEventProcessorRunner};
use std::sync::Arc;
use tokio::sync::watch::Receiver;

/// Store processors as concrete MockEventProcessor instances.
/// This allows access to the fields for testing purposes.
pub struct MockEventProcessorRunner {
    /// The event processors to be used by the runner
    pub event_processors: Vec<Arc<MockEventProcessor>>,
    pub monitored_homeservers_limit: usize,
    pub shutdown_rx: Receiver<bool>,
}

impl MockEventProcessorRunner {
    /// Creates a new instance from the provided event processors
    pub fn new(
        event_processors: Vec<MockEventProcessor>,
        monitored_homeservers_limit: usize,
        shutdown_rx: Receiver<bool>,
    ) -> Self {
        let arcs: Vec<Arc<MockEventProcessor>> =
            event_processors.into_iter().map(Arc::new).collect();

        Self {
            event_processors: arcs,
            monitored_homeservers_limit,
            shutdown_rx,
        }
    }
}

#[async_trait::async_trait]
impl TEventProcessorRunner for MockEventProcessorRunner {
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

    fn monitored_homeservers_limit(&self) -> usize {
        self.monitored_homeservers_limit
    }

    async fn external_homeservers_by_priority(&self) -> Result<Vec<String>, DynError> {
        let persistedhs_ids = Homeserver::get_all_from_graph().await?;

        let mut hs_ids = vec![];

        // Skip the homeserver IDs that are not part of the runner's event processors
        // and exclude the default homeserver, which is processed separately
        for mock_event_processor in self.event_processors.iter() {
            let hs_id = mock_event_processor.homeserver_id.to_string();
            if persistedhs_ids.contains(&hs_id) && hs_id != self.default_homeserver() {
                hs_ids.push(hs_id);
            }
        }

        Ok(hs_ids)
    }

    /// Returns the event processor for the specified homeserver.
    ///
    /// The mock event processor was pre-built and given to the mock runner on initialization, so this returns a reference to it.
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
