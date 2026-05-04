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
    pub monitored_hs_limit: usize,
    pub shutdown_rx: Receiver<bool>,
}

impl MockEventProcessorRunner {
    /// Creates a new instance from the provided event processors
    pub fn new(
        event_processors: Vec<MockEventProcessor>,
        monitored_hs_limit: usize,
        shutdown_rx: Receiver<bool>,
    ) -> Self {
        let arcs: Vec<Arc<MockEventProcessor>> =
            event_processors.into_iter().map(Arc::new).collect();

        Self {
            event_processors: arcs,
            monitored_hs_limit,
            shutdown_rx,
        }
    }
    pub async fn hs_by_priority(&self) -> Result<Vec<String>, DynError> {
        let persisted_hs_ids = Homeserver::get_all_active_from_graph().await?;

        let mut hs_ids = vec![];

        for mock_event_processor in self.event_processors.iter() {
            let hs_id = mock_event_processor.homeserver_id.to_string();
            if persisted_hs_ids.contains(&hs_id) && hs_id != self.default_homeserver() {
                hs_ids.push(hs_id);
            }
        }

        Ok(hs_ids)
    }

    pub fn default_homeserver(&self) -> &str {
        // Use first mock homeserver ID if available, otherwise fallback to mock constant
        self.event_processors
            .first()
            .map(|s| s.homeserver_id.as_str())
            .unwrap_or("8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo")
    }
}

#[async_trait::async_trait]
impl TEventProcessorRunner for MockEventProcessorRunner {
    fn shutdown_rx(&self) -> Receiver<bool> {
        self.shutdown_rx.clone()
    }

    /// Returns the event processor for the specified homeserver.
    ///
    /// The mock event processor was pre-built and given to the mock runner on initialization, so this returns a reference to it.
    async fn build(&self, hs_id: String) -> Result<Arc<dyn TEventProcessor>, DynError> {
        let mock_event_processor = self
            .event_processors
            .iter()
            .find(|p| p.homeserver_id.to_string() == hs_id)
            .cloned()
            .ok_or(format!("No MockEventProcessor for HS ID: {hs_id}"))?;

        Ok(mock_event_processor)
    }

    async fn pre_run(&self) -> Result<Vec<String>, DynError> {
        let hs_ids = self.hs_by_priority().await?;
        let max_index = std::cmp::min(self.monitored_hs_limit, hs_ids.len());
        Ok(hs_ids[..max_index].to_vec())
    }
}
