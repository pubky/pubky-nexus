use nexus_common::types::DynError;
use nexus_watcher::service::ProcessResult;
use nexus_watcher::TEventProcessor;
use tokio::sync::watch::Receiver;
use tokio::time::Duration;

pub struct MockEventProcessor {
    pub processor_status: ProcessResult,
    pub timeout: Option<Duration>,
    pub homeserver_id: String,
}

impl MockEventProcessor {
    pub fn new(processor_status: ProcessResult, timeout: Option<Duration>, homeserver_id: String) -> Self {
        Self { processor_status, timeout, homeserver_id }
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
            ProcessResult::Success(_) => Ok(()),
            ProcessResult::Error(e) => Err(format!("Mock error: {}", e).into()),
            ProcessResult::Panic() => panic!("MockEventProcessor panic for testing")
        }
    }
}