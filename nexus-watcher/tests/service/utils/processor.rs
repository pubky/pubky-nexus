use std::sync::Arc;

use crate::service::utils::MockEventProcessorResult;
use nexus_common::types::DynError;
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
        // Simulate a timeout/long-running work if needed, but be responsive to shutdown
        // This simulates the processing of event lines, which can take a while but can be interrupted by the shutdown signal
        if let Some(timeout) = self.timeout {
            let mut shutdown_rx = self.shutdown_rx.clone();
            tokio::select! {
                _ = tokio::time::sleep(timeout) => {},
                _ = shutdown_rx.changed() => {
                    return Ok(());
                }
            }
        }

        match &self.processor_status {
            MockEventProcessorResult::Success(_) => Ok(()),
            MockEventProcessorResult::Error(e) => Err(format!("{e}").into()),
            MockEventProcessorResult::Panic() => panic!("Event processor panicked: unknown error"),
        }
    }
}
