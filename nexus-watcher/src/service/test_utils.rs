//! Processors for the service's unit tests.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::errors::EventProcessorError;
use crate::events::{Event, EventHandler};
use crate::service::indexer::TEventProcessor;

struct NoopHandler;

#[async_trait::async_trait]
impl EventHandler for NoopHandler {
    async fn handle(&self, _event: &Event) -> Result<(), EventProcessorError> {
        Ok(())
    }
}

/// Processor whose run takes `work` (forever if `None`), under a deadline of `timeout`.
/// It counts the runs it started.
pub(crate) struct SlowProcessor {
    handler: Arc<dyn EventHandler>,
    work: Option<Duration>,
    timeout: Duration,
    starts: AtomicUsize,
}

impl SlowProcessor {
    pub(crate) fn new(work: Option<Duration>, timeout: Duration) -> Self {
        Self {
            handler: Arc::new(NoopHandler),
            work,
            timeout,
            starts: AtomicUsize::new(0),
        }
    }

    pub(crate) fn starts(&self) -> usize {
        self.starts.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl TEventProcessor for SlowProcessor {
    fn event_handler(&self) -> &Arc<dyn EventHandler> {
        &self.handler
    }

    fn instance_name(&self) -> &'static str {
        "SlowProcessor"
    }

    fn custom_timeout(&self) -> Option<Duration> {
        Some(self.timeout)
    }

    async fn run_internal(self: Arc<Self>) -> Result<(), EventProcessorError> {
        self.starts.fetch_add(1, Ordering::SeqCst);
        match self.work {
            Some(work) => tokio::time::sleep(work).await,
            None => std::future::pending().await,
        }
        Ok(())
    }
}
