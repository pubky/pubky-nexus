use crate::service::utils::MockEventProcessorResult;
use nexus_common::types::DynError;
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
    async fn run(self: Box<Self>, _shutdown_rx: Receiver<bool>) -> Result<(), DynError> {
        // Simulate a timeout
        if let Some(timeout) = self.timeout {
            tokio::time::sleep(timeout).await;
        }

        match &self.processor_status {
            MockEventProcessorResult::Success(_) => Ok(()),
            MockEventProcessorResult::Error(e) => Err(format!("Mock error: {}", e).into()),
            MockEventProcessorResult::Panic() => panic!("MockEventProcessor panic for testing"),
        }
    }
}
