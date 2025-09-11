use crate::service::utils::processor::MockEventProcessor;
use indexmap::IndexMap;
use nexus_common::types::DynError;
use nexus_watcher::service::{TEventProcessor, TEventProcessorFactory, PROCESSING_TIMEOUT_SECS};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch::Receiver;

/// Store processors as concrete MockEventProcessor instances.
/// This allows access to the fields for testing purposes.
pub struct MockEventProcessorFactory {
    /// The event processors to be used by the factory
    /// TODO: https://github.com/pubky/pubky-nexus/issues/564
    pub event_processors: IndexMap<String, Arc<MockEventProcessor>>,
    pub timeout: Option<Duration>,
    pub shutdown_rx: Receiver<bool>,
}

impl MockEventProcessorFactory {
    /// Creates a new factory instance from the provided event processors
    pub fn new(
        event_processors: IndexMap<String, MockEventProcessor>,
        timeout: Option<Duration>,
        shutdown_rx: Receiver<bool>,
    ) -> Self {
        let arcs: IndexMap<String, Arc<MockEventProcessor>> = event_processors
            .into_iter()
            .map(|(k, v)| (k, Arc::new(v)))
            .collect();

        Self {
            event_processors: arcs,
            timeout,
            shutdown_rx,
        }
    }
}

#[async_trait::async_trait]
impl TEventProcessorFactory for MockEventProcessorFactory {
    /// Returns the timeout applied for each event processor run
    fn timeout(&self) -> Duration {
        self.timeout
            .unwrap_or(Duration::from_secs(PROCESSING_TIMEOUT_SECS))
    }

    fn shutdown_rx(&self) -> Receiver<bool> {
        self.shutdown_rx.clone()
    }

    fn default_homeserver(&self) -> &str {
        // Use first mock homeserver ID if available, otherwise fallback to mock constant
        self.event_processors.keys().next()
            .map(|s| s.as_str())
            .unwrap_or("8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo")
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
