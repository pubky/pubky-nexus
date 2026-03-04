use crate::service::utils::MockEventProcessorRunner;
use nexus_common::types::DynError;
use nexus_watcher::service::{ProcessedStats, TEventProcessor, TEventProcessorRunner};
use std::sync::Arc;
use tokio::sync::watch::Receiver;

/// Wraps a [`MockEventProcessorRunner`] but panics in `run_default_homeserver()`.
/// Used to test that a panic in the default HS task is detected by the `JoinSet`
/// and causes the external HS task to stop via the internal shutdown channel.
pub struct PanickingDefaultHsRunner {
    pub inner: MockEventProcessorRunner,
}

#[async_trait::async_trait]
impl TEventProcessorRunner for PanickingDefaultHsRunner {
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
        panic!("simulated default HS task crash");
    }
}

/// Wraps a [`MockEventProcessorRunner`] but panics in `run_external_homeservers()`.
/// Used to test that a panic in the external HS task is detected by the `JoinSet`
/// and causes the default HS task to stop via the internal shutdown channel.
pub struct PanickingExternalHsRunner {
    pub inner: MockEventProcessorRunner,
}

#[async_trait::async_trait]
impl TEventProcessorRunner for PanickingExternalHsRunner {
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

    async fn run_external_homeservers(&self) -> Result<ProcessedStats, DynError> {
        panic!("simulated external HS task crash");
    }
}
