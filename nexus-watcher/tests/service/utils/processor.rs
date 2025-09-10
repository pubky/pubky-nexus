use std::sync::Arc;

use crate::service::utils::MockEventProcessorResult;
use nexus_common::types::DynError;
use nexus_watcher::events::errors::EventProcessorError;
use nexus_watcher::events::TEventProcessor;
use tokio::sync::watch::Receiver;
use tokio::time::Duration;

pub struct MockEventProcessor {
    pub processor_status: MockEventProcessorResult,
    pub timeout: Option<Duration>,
    pub homeserver_id: String,
    pub shutdown_rx: Receiver<bool>,
}

#[async_trait::async_trait]
impl TEventProcessor for MockEventProcessor {
    async fn run(self: Arc<Self>) -> Result<(), DynError> {
        // If shutdown was already requested, exit immediately so callers can count it
        if *self.shutdown_rx.borrow() {
            return Err(EventProcessorError::ShutdownRequested.into());
        }

        // Simulate a timeout/long-running work if needed, but be responsive to shutdown
        if let Some(timeout) = self.timeout {
            let mut shutdown_rx = self.shutdown_rx.clone();
            tokio::select! {
                _ = tokio::time::sleep(timeout) => {},
                _ = shutdown_rx.changed() => {
                    return Err(EventProcessorError::ShutdownRequested.into());
                }
            }
        }

        // Check again before returning a result
        if *self.shutdown_rx.borrow() {
            return Err(EventProcessorError::ShutdownRequested.into());
        }

        match &self.processor_status {
            MockEventProcessorResult::Success(_) => Ok(()),
            MockEventProcessorResult::Error(e) => Err(format!("{e}").into()),
            MockEventProcessorResult::Panic() => panic!("Event processor panicked: unknown error"),
        }
    }
}
