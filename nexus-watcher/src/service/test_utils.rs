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
/// It counts the runs it started, and the most of them going at once.
pub(crate) struct SlowProcessor {
    handler: Arc<dyn EventHandler>,
    work: Option<Duration>,
    timeout: Duration,
    starts: AtomicUsize,
    running: AtomicUsize,
    peak: AtomicUsize,
}

impl SlowProcessor {
    pub(crate) fn new(work: Option<Duration>, timeout: Duration) -> Self {
        Self {
            handler: Arc::new(NoopHandler),
            work,
            timeout,
            starts: AtomicUsize::new(0),
            running: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        }
    }

    pub(crate) fn starts(&self) -> usize {
        self.starts.load(Ordering::SeqCst)
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
        self.starts.fetch_add(1, Ordering::SeqCst);
        let running = self.running.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(running, Ordering::SeqCst);
        match self.work {
            Some(work) => tokio::time::sleep(work).await,
            None => std::future::pending().await,
        }
        self.running.fetch_sub(1, Ordering::SeqCst);
        Ok(())
    }
}
