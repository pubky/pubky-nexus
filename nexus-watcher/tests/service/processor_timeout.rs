//! A processor run that exceeds its timeout is not cancelled: it finishes in the background,
//! holding its in-flight mark until it does.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use nexus_watcher::errors::EventProcessorError;
use nexus_watcher::events::EventHandler;
use nexus_watcher::service::indexer::{RunError, TEventProcessor};
use nexus_watcher::service::runner::InFlightRuns;

use crate::service::utils::create_mock_handler;

/// Processor that takes `work` to process its batch, under a deadline of `timeout`.
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

/// A timed-out run returns `TimedOut` at once, then finishes its work in the background, so
/// a batch that takes longer than the deadline still completes. Its mark is held until then,
/// which keeps the next run for its homeserver from starting next to it.
#[tokio_shared_rt::test(shared)]
async fn test_timed_out_run_finishes_in_the_background() -> Result<()> {
    let runs = InFlightRuns::default();
    let finished = Arc::new(AtomicUsize::new(0));
    let processor = Arc::new(SlowBatch {
        handler: create_mock_handler(Ok(()), None),
        work: Duration::from_millis(400),
        timeout: Duration::from_millis(100),
        finished: finished.clone(),
    });

    let mark = runs.try_start("hs").expect("nothing runs yet");
    let result = processor.run_holding(Some(mark)).await;

    assert!(matches!(result, Err(RunError::TimedOut)), "got {result:?}");
    assert_eq!(finished.load(Ordering::SeqCst), 0);
    assert!(
        runs.try_start("hs").is_none(),
        "the timed-out run must still hold its mark"
    );

    // Long enough for the run to finish its work.
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(
        finished.load(Ordering::SeqCst),
        1,
        "the timed-out run must finish its work"
    );
    assert!(
        runs.try_start("hs").is_some(),
        "the mark must be released once the run ends"
    );
    Ok(())
}
