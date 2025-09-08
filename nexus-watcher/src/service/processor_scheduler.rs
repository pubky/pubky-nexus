use crate::events::errors::EventProcessorError;
use crate::events::{TEventProcessor, TEventProcessorFactory};
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use std::result::Result;
use std::sync::Arc;
use tokio::sync::watch::Receiver;
use tokio::time::{timeout, Duration};
use tracing::error;

/// The result type for the event processor pool
type ProcessorResultType = Result<(u64, u64), DynError>;

/// This struct is used to pool event processors (homeservers) for the rolling window
pub struct ProcessorScheduler {
    event_processor_factory: Arc<dyn TEventProcessorFactory>,
}

impl ProcessorScheduler {
    /// Creates a new event processor pool
    pub fn new(event_processor_factory: Arc<dyn TEventProcessorFactory>) -> Self {
        Self {
            event_processor_factory,
        }
    }

    /// Returns the timeout for the event processor
    pub fn timeout(&self) -> Duration {
        self.event_processor_factory.timeout()
    }

    /// Returns the shutdown receiver for the event processor
    pub fn shutdown_rx(&self) -> Receiver<bool> {
        self.event_processor_factory.shutdown_rx()
    }

    /// Builds an event processor from a homeserver ID
    async fn build_event_processor_from(
        &self,
        hs_id: String,
    ) -> Result<Box<dyn TEventProcessor>, DynError> {
        self.event_processor_factory.build(hs_id).await
    }

    /// Runs the processor scheduler
    pub async fn run(&self) -> ProcessorResultType {
        let hs_ids = Homeserver::get_all_from_graph()
            .await
            .expect("No Homeserver IDs found in graph");

        let mut processed_homeservers = 0;
        let mut skipped_homeservers = 0;

        for hs_id in hs_ids {
            let Ok(event_processor) = self.build_event_processor_from(hs_id.clone()).await else {
                error!("Failed to build event processor for homeserver: {}", hs_id);
                continue;
            };
            match timeout(self.timeout(), event_processor.run(self.shutdown_rx())).await {
                Ok(Ok(_)) => processed_homeservers += 1,
                Ok(Err(e)) => {
                    if let Some(EventProcessorError::ShutdownRequested) =
                        e.as_ref().downcast_ref::<EventProcessorError>()
                    {
                        skipped_homeservers += 1;
                        continue;
                    }
                    error!("Event processor failed for {}: {:?}", hs_id, e);
                    skipped_homeservers += 1;
                }
                Err(_) => {
                    error!("Event processor timed out for {}", hs_id);
                    skipped_homeservers += 1;
                }
            }
        }

        Ok((processed_homeservers, skipped_homeservers))
    }
}
