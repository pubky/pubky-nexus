use nexus_common::types::DynError;
use nexus_watcher::service::{ProcessedStats, TEventProcessor, TEventProcessorRunner};
use std::sync::Arc;
use tokio::sync::watch::Receiver;

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
