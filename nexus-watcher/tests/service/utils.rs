use nexus_common::types::DynError;
use nexus_watcher::service::ProcessResult;
use tokio::task::JoinError;
use tokio::time::Duration;

pub struct MockEventProcessor {
    pub processor_status: ProcessResult,
    pub timeout: Option<Duration>,
    pub homeserver_id: String,
}

impl MockEventProcessor {
    pub fn new(processor_status: ProcessResult, homeserver_id: String) -> Self {
        Self { processor_status, timeout: None, homeserver_id }
    }

    pub async fn run(&self) -> Result<Result<(), DynError>, JoinError> {
        // Simulate a timeout 
        if let Some(timeout) = self.timeout {
            tokio::time::sleep(timeout).await;
        }

        match &self.processor_status {
            ProcessResult::Success => Ok(Ok(())),
            ProcessResult::Error(e) => Ok(Err(format!("Mock error: {}", e).into())),
            ProcessResult::Panic(_join_error) => panic!("Mock panic for testing")
        }
    }
}