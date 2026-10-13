//! A processor run that exceeds its timeout is cancelled, not left running.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use nexus_watcher::errors::EventProcessorError;
use nexus_watcher::events::EventHandler;
use nexus_watcher::service::indexer::{RunError, TEventProcessor};

use crate::service::utils::create_mock_handler;

/// Counts a run as alive until its future is dropped, which is what cancelling its task does.
struct Alive(Arc<AtomicUsize>);

impl Alive {
    fn new(count: &Arc<AtomicUsize>) -> Self {
        count.fetch_add(1, Ordering::SeqCst);
        Self(count.clone())
    }
}

impl Drop for Alive {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Processor that takes `work` to process its batch, under a deadline of `timeout`.
struct SlowBatch {
    handler: Arc<dyn EventHandler>,
    work: Duration,
    timeout: Duration,
    alive: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
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
        let _alive = Alive::new(&self.alive);
        self.peak
            .fetch_max(self.alive.load(Ordering::SeqCst), Ordering::SeqCst);
        tokio::time::sleep(self.work).await;
        self.finished.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// A timed-out run is cancelled at its deadline: it never finishes its work, and it is gone
/// by the time `run` returns, so the next tick's run, starting from the same cursor, cannot
/// overlap it.
#[tokio_shared_rt::test(shared)]
async fn test_timed_out_run_is_cancelled() -> Result<()> {
    let handler: Arc<dyn EventHandler> = create_mock_handler(Ok(()), None);
    let alive = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let finished = Arc::new(AtomicUsize::new(0));
    let tick = || {
        Arc::new(SlowBatch {
            handler: handler.clone(),
            work: Duration::from_millis(400),
            timeout: Duration::from_millis(100),
            alive: alive.clone(),
            peak: peak.clone(),
            finished: finished.clone(),
        })
    };

    for _ in 0..2 {
        let result = tick().run().await;
        assert!(matches!(result, Err(RunError::TimedOut)), "got {result:?}");
        assert_eq!(
            alive.load(Ordering::SeqCst),
            0,
            "the timed-out run must be gone once run returns"
        );
    }

    // Long enough for a run that survived its deadline to finish its work.
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(
        finished.load(Ordering::SeqCst),
        0,
        "no timed-out run may finish its work"
    );
    assert_eq!(peak.load(Ordering::SeqCst), 1, "the runs must not overlap");
    Ok(())
}
