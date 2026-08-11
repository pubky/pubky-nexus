use crate::service::utils::create_mock_handler;
use crate::service::utils::{create_mock_event_processors, MockEventProcessorRunner, HS_IDS};
use anyhow::Result;
use nexus_common::types::DynError;
use nexus_watcher::errors::EventProcessorError;
use nexus_watcher::events::EventHandler;
use nexus_watcher::service::TEventProcessor;
use nexus_watcher::service::TEventProcessorRunner;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(2);

#[tokio_shared_rt::test(shared)]
async fn test_mock_event_processors() -> Result<(), DynError> {
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let mock_processors = create_mock_event_processors(Some(TIMEOUT), shutdown_rx.clone());
    let runner = MockEventProcessorRunner::new(mock_processors, HS_IDS.len(), shutdown_rx);

    // Test successful event processor
    let ev_processor_0 = runner.build(HS_IDS[0]).await?;
    assert!(ev_processor_0.run().await.is_ok());

    // Test error event processor
    let ev_processor_1 = runner.build(HS_IDS[1]).await?;
    assert!(ev_processor_1.run().await.is_err());

    // Test panic event processor
    let ev_processor_2 = runner.build(HS_IDS[2]).await?;
    let ev_processor_2_res = ev_processor_2.run().await;
    assert!(ev_processor_2_res.is_err() && ev_processor_2_res.unwrap_err().is_panic());

    // Test timeout scenarios
    let ev_processor_3 = runner.build(HS_IDS[3]).await?;
    let ev_processor_3_res = ev_processor_3.run().await;
    assert!(ev_processor_3_res.is_err() && ev_processor_3_res.unwrap_err().is_timeout());

    let ev_processor_4 = runner.build(HS_IDS[4]).await?;
    assert!(
        ev_processor_4.run().await.is_ok(),
        "Event processor should not timeout"
    );

    Ok(())
}

/// Verifies that `run` waits for its timed-out task to be cancelled.
#[tokio_shared_rt::test(shared)]
async fn processor_run_aborts_spawned_task_on_timeout() -> Result<(), DynError> {
    let task_dropped = Arc::new(AtomicBool::new(false));
    let processor = Arc::new(AbortTrackingProcessor {
        task_dropped: task_dropped.clone(),
        event_handler: create_mock_handler(Ok(()), None),
        timeout: Duration::from_millis(50),
    });

    let err = processor.run().await.unwrap_err();
    assert!(err.is_timeout(), "expected timeout, got {err:?}");
    assert!(
        task_dropped.load(Ordering::SeqCst),
        "run should await cancellation of the timed-out task"
    );

    Ok(())
}

struct AbortTrackingProcessor {
    task_dropped: Arc<AtomicBool>,
    event_handler: Arc<dyn EventHandler>,
    timeout: Duration,
}

struct TaskDropGuard(Arc<AtomicBool>);

impl Drop for TaskDropGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[async_trait::async_trait]
impl TEventProcessor for AbortTrackingProcessor {
    fn event_handler(&self) -> &Arc<dyn EventHandler> {
        &self.event_handler
    }

    fn instance_name(&self) -> &'static str {
        "AbortTrackingProcessor"
    }

    fn custom_timeout(&self) -> Option<Duration> {
        Some(self.timeout)
    }

    async fn run_internal(self: Arc<Self>) -> Result<(), EventProcessorError> {
        let _guard = TaskDropGuard(self.task_dropped.clone());
        std::future::pending::<()>().await;
        Ok(())
    }
}
