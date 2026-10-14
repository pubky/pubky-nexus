//! A processor run that outlives its timeout is waited for, not left running in the background.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use nexus_watcher::errors::EventProcessorError;
use nexus_watcher::events::EventHandler;
use nexus_watcher::service::indexer::TEventProcessor;
use tokio::time::Instant;

use crate::service::utils::create_mock_handler;

/// Processor that takes `work` to process its batch, under a timeout of `timeout`.
struct SlowBatch {
    handler: Arc<dyn EventHandler>,
    work: Duration,
    timeout: Duration,
    finished: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl TEventProcessor for SlowBatch {
    fn event_handler(&self) -> &Arc<dyn EventHandler> {
        &self.handler
    }

    fn instance_name(&self) -> &'static str {
        "SlowBatch"
    }

    fn custom_timeout(&self) -> Option<Duration> {
        Some(self.timeout)
    }

    async fn run_internal(self: Arc<Self>) -> Result<(), EventProcessorError> {
        tokio::time::sleep(self.work).await;
        self.finished.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// `run` returns only once the run has finished its work, with its result, even when the
/// work takes longer than the timeout. A caller that starts the next run after `run`
/// returned therefore never runs a homeserver twice at once.
#[tokio::test(start_paused = true)]
async fn test_run_past_its_timeout_is_waited_for() -> Result<()> {
    let finished = Arc::new(AtomicUsize::new(0));
    let processor = Arc::new(SlowBatch {
        handler: create_mock_handler(Ok(()), None),
        work: Duration::from_millis(400),
        timeout: Duration::from_millis(100),
        finished: finished.clone(),
    });

    let started = Instant::now();
    let result = processor.run().await;

    assert!(result.is_ok(), "got {result:?}");
    assert_eq!(
        finished.load(Ordering::SeqCst),
        1,
        "run must return only once the work is done"
    );
    assert!(started.elapsed() >= Duration::from_millis(400));
    Ok(())
}
