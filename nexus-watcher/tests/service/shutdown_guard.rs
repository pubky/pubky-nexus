use crate::service::utils::{
    create_random_homeservers_and_persist, setup, MockEventProcessorResult,
    MockEventProcessorRunner, PanickingDefaultHsRunner, PanickingExternalHsRunner,
};
use anyhow::Result;
use nexus_watcher::service::NexusWatcher;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Test A: Sending an external shutdown signal causes `run_tasks` to return `Ok`.
/// Verifies the forwarder task bridges the external signal into the internal channel,
/// causing all processing tasks to exit gracefully.
#[tokio_shared_rt::test(shared)]
async fn test_run_tasks_clean_shutdown_via_external_signal() -> Result<()> {
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
    let runner = Arc::new(runner);

    tokio::spawn(async move {
        sleep(Duration::from_millis(500)).await;
        let _ = shutdown_tx.send(true);
    });

    let result = NexusWatcher::run_tasks(shutdown_rx, runner, 100).await;
    assert!(result.is_ok(), "run_tasks should return Ok on clean shutdown");

    Ok(())
}

/// Test B: A panic in `run_default_homeserver` causes the spawned task to crash.
/// `JoinSet::join_next` surfaces the panic, then the remaining tasks are signalled
/// to stop via the internal shutdown channel.
/// `run_tasks` returns `Err` because the panicked `JoinError` is observed during drain.
#[tokio_shared_rt::test(shared)]
async fn test_run_tasks_default_hs_panic_propagates_via_guard() -> Result<()> {
    let mut event_processor_list = setup().await?;
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

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

    let inner = MockEventProcessorRunner::new(event_processor_list, 3, shutdown_rx.clone());
    let runner = Arc::new(PanickingDefaultHsRunner { inner });

    let result = NexusWatcher::run_tasks(shutdown_rx, runner, 100).await;
    assert!(
        result.is_err(),
        "run_tasks should return Err when the default HS task panics"
    );

    Ok(())
}

/// Test C: A panic in `run_external_homeservers` causes the spawned task to crash.
/// `JoinSet::join_next` surfaces the panic, then the remaining tasks are signalled
/// to stop via the internal shutdown channel.
/// `run_tasks` returns `Err` because the panicked `JoinError` is observed during drain.
#[tokio_shared_rt::test(shared)]
async fn test_run_tasks_external_hs_panic_propagates_via_guard() -> Result<()> {
    let mut event_processor_list = setup().await?;
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

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

    let inner = MockEventProcessorRunner::new(event_processor_list, 3, shutdown_rx.clone());
    let runner = Arc::new(PanickingExternalHsRunner { inner });

    let result = NexusWatcher::run_tasks(shutdown_rx, runner, 100).await;
    assert!(
        result.is_err(),
        "run_tasks should return Err when the external HS task panics"
    );

    Ok(())
}
