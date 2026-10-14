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

/// Processor whose run takes `work`, under a timeout of `timeout`. It counts the most of its
/// runs going at once.
pub(crate) struct SlowProcessor {
    handler: Arc<dyn EventHandler>,
    work: Duration,
    timeout: Duration,
    running: AtomicUsize,
    peak: AtomicUsize,
}

impl SlowProcessor {
    pub(crate) fn new(work: Duration, timeout: Duration) -> Self {
        Self {
            handler: Arc::new(NoopHandler),
            work,
            timeout,
            running: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        }
    }

    /// The most runs that were going at once.
    pub(crate) fn peak(&self) -> usize {
        self.peak.load(Ordering::SeqCst)
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
        let running = self.running.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(running, Ordering::SeqCst);
        tokio::time::sleep(self.work).await;
        self.running.fetch_sub(1, Ordering::SeqCst);
        Ok(())
    }
}
