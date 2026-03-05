use anyhow::Result;
use nexus_common::types::DynError;
use nexus_watcher::service::{
    NexusWatcher, ProcessedStats, TEventProcessor, TEventProcessorRunner,
};
use std::{sync::Arc, time::Duration};
use tokio::sync::watch::Receiver;

use crate::service::utils::{
    create_random_homeservers_and_persist, MockEventProcessorResult, MockEventProcessorRunner,
};

/// Controls which processing loop should panic in [`PanickingRunner`].
///
/// This lets tests reuse one runner implementation for both panic paths.
pub enum PanicTarget {
    DefaultHs,
    ExternalHs,
}

pub struct PanickingRunner {
    pub panic_target: PanicTarget,
    pub inner: Arc<dyn TEventProcessorRunner>,
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
        if matches!(self.panic_target, PanicTarget::DefaultHs) {
            panic!("simulated default HS task crash");
        }
        self.inner.run_default_homeserver().await
    }

    async fn run_external_homeservers(&self) -> Result<ProcessedStats, DynError> {
        if matches!(self.panic_target, PanicTarget::ExternalHs) {
            panic!("simulated external HS task crash");
        }
        self.inner.run_external_homeservers().await
    }
}

/// Ensures `run_tasks` returns promptly with `Err` if a HS-processing task panics.
#[tokio_shared_rt::test(shared)]
async fn test_panicking_external_task_does_not_hang_run_tasks() -> Result<()> {
    let mut event_processor_list = crate::service::utils::setup().await?;
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Create 3 random homeservers with success result with defined sleep_duration
    for _ in 0..3 {
        create_random_homeservers_and_persist(
            &mut event_processor_list,
            Some(Duration::from_secs(30)),
            MockEventProcessorResult::Success,
            None,
            shutdown_rx.clone(),
        )
        .await;
    }

    let inner_runner = MockEventProcessorRunner::new(event_processor_list, 3, shutdown_rx.clone());
    let inner = Arc::new(inner_runner);

    // Runner that will panic when processing external HSs
    let runner_panic_external = Arc::new(PanickingRunner {
        panic_target: PanicTarget::ExternalHs,
        inner: inner.clone(),
    });
    assert_run_tasks(shutdown_rx.clone(), runner_panic_external).await?;

    // Runner that will panic when processing default HS
    let runner_panic_default = Arc::new(PanickingRunner {
        panic_target: PanicTarget::DefaultHs,
        inner,
    });
    assert_run_tasks(shutdown_rx.clone(), runner_panic_default).await?;

    Ok(())
}

async fn assert_run_tasks(shutdown_rx: Receiver<bool>, runner: Arc<PanickingRunner>) -> Result<()> {
    let res = tokio::time::timeout(
        Duration::from_secs(2),
        NexusWatcher::run_tasks(shutdown_rx, runner, 100),
    )
    .await;
    assert!(res.is_ok(), "run_tasks should not hang on task panic");
    assert!(res.unwrap().is_err(), "run_tasks should Err on task panic");

    Ok(())
}
