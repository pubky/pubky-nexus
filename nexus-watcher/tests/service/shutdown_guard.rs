use crate::service::utils::{
    create_random_homeservers_and_persist, setup, MockEventProcessorResult,
    MockEventProcessorRunner,
};
use anyhow::Result;
use nexus_common::types::DynError;
use nexus_watcher::service::{
    NexusWatcher, ProcessedStats, TEventProcessor, TEventProcessorRunner,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch::Receiver;
use tokio::time::sleep;

const WATCHER_SLEEP_MS: u64 = 100;
const SHUTDOWN_DELAY_MS: u64 = 500;

/// Controls which processing path should panic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PanicTarget {
    DefaultHomeserver,
    ExternalHomeservers,
}

/// Wraps a [`MockEventProcessorRunner`] and panics in one selected run method.
struct PanickingRunner {
    inner: MockEventProcessorRunner,
    panic_target: PanicTarget,
}

#[async_trait::async_trait]
impl TEventProcessorRunner for PanickingRunner {
    fn shutdown_rx(&self) -> Receiver<bool> {
        self.inner.shutdown_rx()
    }

    fn default_homeserver(&self) -> &str {
        self.inner.default_homeserver()
    }

    fn monitored_homeservers_limit(&self) -> usize {
        self.inner.monitored_homeservers_limit()
    }

    async fn external_homeservers_by_priority(&self) -> Result<Vec<String>, DynError> {
        self.inner.external_homeservers_by_priority().await
    }

    async fn build(&self, homeserver_id: String) -> Result<Arc<dyn TEventProcessor>, DynError> {
        self.inner.build(homeserver_id).await
    }

    async fn run_default_homeserver(&self) -> Result<ProcessedStats, DynError> {
        if matches!(self.panic_target, PanicTarget::DefaultHomeserver) {
            panic!("simulated default HS task crash");
        }
        self.inner.run_default_homeserver().await
    }

    async fn run_external_homeservers(&self) -> Result<ProcessedStats, DynError> {
        if matches!(self.panic_target, PanicTarget::ExternalHomeservers) {
            panic!("simulated external HS task crash");
        }
        self.inner.run_external_homeservers().await
    }
}

async fn setup_runner_and_channel() -> Result<(
    MockEventProcessorRunner,
    tokio::sync::watch::Sender<bool>,
    tokio::sync::watch::Receiver<bool>,
)> {
    let mut event_processor_list = setup().await?;
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    for _ in 0..3 {
        create_random_homeservers_and_persist(
            &mut event_processor_list,
            None,
            MockEventProcessorResult::Success,
            None,
            shutdown_rx.clone(),
        )
        .await;
    }

    let runner = MockEventProcessorRunner::new(event_processor_list, 3, shutdown_rx.clone());
    Ok((runner, shutdown_tx, shutdown_rx))
}

async fn assert_panic_propagates(panic_target: PanicTarget) -> Result<()> {
    let (inner, _shutdown_tx, shutdown_rx) = setup_runner_and_channel().await?;
    let runner = Arc::new(PanickingRunner {
        inner,
        panic_target,
    });

    let result = NexusWatcher::run_tasks(shutdown_rx, runner, WATCHER_SLEEP_MS).await;
    assert!(
        result.is_err(),
        "run_tasks should return Err when a task panics"
    );
    Ok(())
}

/// Test A: Sending an external shutdown signal causes `run_tasks` to return `Ok`.
/// Verifies the forwarder task bridges the external signal into the internal channel,
/// causing all processing tasks to exit gracefully.
#[tokio_shared_rt::test(shared)]
async fn test_run_tasks_clean_shutdown_via_external_signal() -> Result<()> {
    let (runner, shutdown_tx, shutdown_rx) = setup_runner_and_channel().await?;
    let runner = Arc::new(runner);

    tokio::spawn(async move {
        sleep(Duration::from_millis(SHUTDOWN_DELAY_MS)).await;
        let _ = shutdown_tx.send(true);
    });

    let result = NexusWatcher::run_tasks(shutdown_rx, runner, WATCHER_SLEEP_MS).await;
    assert!(
        result.is_ok(),
        "run_tasks should return Ok on clean shutdown"
    );
    Ok(())
}

/// Test B: A panic in `run_default_homeserver` causes the spawned task to crash.
/// `JoinSet::join_next` surfaces the panic, then the remaining tasks are signalled
/// to stop via the internal shutdown channel.
/// `run_tasks` returns `Err` because the panicked `JoinError` is observed during drain.
#[tokio_shared_rt::test(shared)]
async fn test_run_tasks_default_hs_panic_propagates_via_guard() -> Result<()> {
    assert_panic_propagates(PanicTarget::DefaultHomeserver).await
}

/// Test C: A panic in `run_external_homeservers` causes the spawned task to crash.
/// `JoinSet::join_next` surfaces the panic, then the remaining tasks are signalled
/// to stop via the internal shutdown channel.
/// `run_tasks` returns `Err` because the panicked `JoinError` is observed during drain.
#[tokio_shared_rt::test(shared)]
async fn test_run_tasks_external_hs_panic_propagates_via_guard() -> Result<()> {
    assert_panic_propagates(PanicTarget::ExternalHomeservers).await
}
