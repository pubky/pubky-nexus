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
}

impl MockEventProcessor {
    pub fn new(
        processor_status: MockEventProcessorResult,
        timeout: Option<Duration>,
        homeserver_id: String,
    ) -> Self {
        Self {
            processor_status,
            timeout,
            homeserver_id,
        }
    }
}

#[async_trait::async_trait]
impl TEventProcessor for MockEventProcessor {
    async fn run(self: Box<Self>, mut shutdown_rx: Receiver<bool>) -> Result<(), DynError> {
        // If shutdown was already requested, exit immediately so callers can count it
        if *shutdown_rx.borrow() {
            return Err(EventProcessorError::ShutdownRequested.into());
        }

        // Simulate a timeout/long-running work if needed, but be responsive to shutdown
        if let Some(timeout) = self.timeout {
            tokio::select! {
                _ = tokio::time::sleep(timeout) => {},
                _ = shutdown_rx.changed() => {
                    return Err(EventProcessorError::ShutdownRequested.into());
                }
            }
        }

        // Check again before returning a result
        if *shutdown_rx.borrow() {
            return Err(EventProcessorError::ShutdownRequested.into());
        }

        match &self.processor_status {
            MockEventProcessorResult::Success(_) => Ok(()),
            MockEventProcessorResult::Error(e) => Err(format!("{e}").into()),
            MockEventProcessorResult::Panic() => panic!("Event processor panicked: unknown error"),
        }
    }
}
