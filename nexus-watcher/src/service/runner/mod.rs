mod homeserver;
mod key_based;
mod key_based_hs_backoff;
mod key_based_user_backoff;

pub use homeserver::HsEventProcessorRunner;
pub use key_based::KeyBasedEventProcessorRunner;
pub use key_based_hs_backoff::HomeserverBackoff;
pub use key_based_user_backoff::UserNotFoundBackoff;

use std::sync::Arc;
use std::time::{Duration, Instant};

use nexus_common::types::DynError;
use tokio::sync::watch::Receiver;
use tracing::{debug, error, info};

use crate::service::{
    indexer::{RunError, TEventProcessor},
    stats::{ProcessedStats, ProcessorRunStatus, RunAllProcessorsStats},
};

pub fn status_from_run_result(result: Result<(), RunError>) -> ProcessorRunStatus {
    match result {
        Ok(_) => ProcessorRunStatus::Ok,
        Err(RunError::Internal(_)) => ProcessorRunStatus::Error,
        Err(RunError::Panicked) => ProcessorRunStatus::Panic,
        Err(RunError::TimedOut) => ProcessorRunStatus::Timeout,
    }
}

/// The orchestrator that helps build and run event processors in the Watcher service.
///
/// # Implementation Notes
/// - The `build` method should create and return a fully configured event processor ready for immediate use
/// - Implementors should ensure that created processors are properly isolated and don't share mutable state unless explicitly intended
#[async_trait::async_trait]
pub trait TEventProcessorRunner: Send + Sync {
    /// Returns the shutdown signal receiver
    fn shutdown_rx(&self) -> Receiver<bool>;

    /// Creates and returns a new event processor instance for the specified homeserver.
    ///
    /// # Parameters
    /// * `hs_id` - The homeserver PubkyId. Represents the homeserver this event processor will
    ///   fetch and process events from.
    ///
    /// # Returns
    /// A reference to the event processor instance, ready to be executed with its `run` method.
    ///
    /// # Errors
    /// Returns an error if the event processor couldn't be built
    async fn build(&self, hs_id: &str) -> Result<Arc<dyn TEventProcessor>, DynError>;

    /// Pre-processing step before the main run loop.
    ///
    /// Determines the list of target HS IDs to process in this run cycle, ordered by priority.
    ///
    /// The list is not truncated here: [`Self::poll_limit`] is applied in [`Self::run`], after
    /// the HS IDs in backoff are skipped.
    async fn pre_run(&self) -> Result<Vec<String>, DynError>;

    /// Maximum number of HS IDs built and run per cycle. HS IDs skipped by backoff don't count.
    ///
    /// No limit by default. Runners that cap the targets per run should overwrite this.
    fn poll_limit(&self) -> usize {
        usize::MAX
    }

    /// Post-processing of the run results.
    ///
    /// No-op default implementation. Callers that perform post-processing should overwrite this.
    async fn post_run(&self, stats: RunAllProcessorsStats) -> ProcessedStats {
        ProcessedStats(stats)
    }

    /// Main run loop: builds and runs event processors for the relevant targets.
    ///
    /// # Returns
    /// Statistics about the event processor run results, summarized as [`ProcessedStats`]
    async fn run(&self) -> Result<ProcessedStats, DynError> {
        let hs_ids = self.pre_run().await?;
        let poll_limit = self.poll_limit();
        let mut polled = 0;
        let mut run_stats = RunAllProcessorsStats::default();

        for hs_id in hs_ids {
            if polled >= poll_limit {
                break;
            }

            if *self.shutdown_rx().borrow() {
                info!(homeserver = %hs_id, "Shutdown detected; exiting run loop");
                break;
            }

            if self.backoff_hs_should_skip(&hs_id).await {
                debug!(homeserver = %hs_id, "Skipping homeserver in backoff");
                run_stats.add_run_result(hs_id, Duration::ZERO, ProcessorRunStatus::Skipped);
                continue;
            }

            polled += 1;
            let t0 = Instant::now();
            let status = match self.build(&hs_id).await {
                Ok(event_processor) => status_from_run_result(event_processor.run().await),
                Err(e) => {
                    error!(homeserver = %hs_id, error = %e, "Failed to build event processor");
                    ProcessorRunStatus::FailedToBuild
                }
            };
            let duration = t0.elapsed();

            self.backoff_hs_record_result(&hs_id, &status).await;
            run_stats.add_run_result(hs_id, duration, status);
        }

        let processed_stats = self.post_run(run_stats).await;
        Ok(processed_stats)
    }

    /// Called before processing a HS, to check if backoff mechanism indicates it should be skipped.
    ///
    /// No-op default implementation. Runners that use backoff should overwrite as needed.
    async fn backoff_hs_should_skip(&self, _hs_id: &str) -> bool {
        false
    }

    /// Called after a HS is processed (build + run), to update its backoff status.
    ///
    /// No-op default implementation. Runners that use backoff should overwrite as needed.
    async fn backoff_hs_record_result(&self, _hs_id: &str, _status: &ProcessorRunStatus) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::Mutex;

    /// Runner over fixed HS IDs that records which ones it was asked to build.
    struct RecordingRunner {
        hs_ids: Vec<String>,
        poll_limit: usize,
        backoff: Mutex<HomeserverBackoff>,
        built: Mutex<Vec<String>>,
        shutdown_rx: Receiver<bool>,
    }

    impl RecordingRunner {
        fn new(hs_ids: &[&str], poll_limit: usize) -> Self {
            Self {
                hs_ids: hs_ids.iter().map(|id| id.to_string()).collect(),
                poll_limit,
                backoff: Mutex::new(HomeserverBackoff::default()),
                built: Mutex::new(vec![]),
                shutdown_rx: tokio::sync::watch::channel(false).1,
            }
        }
    }

    #[async_trait::async_trait]
    impl TEventProcessorRunner for RecordingRunner {
        fn shutdown_rx(&self) -> Receiver<bool> {
            self.shutdown_rx.clone()
        }

        async fn build(&self, hs_id: &str) -> Result<Arc<dyn TEventProcessor>, DynError> {
            self.built.lock().await.push(hs_id.to_string());
            Err("not built in this test".into())
        }

        async fn pre_run(&self) -> Result<Vec<String>, DynError> {
            Ok(self.hs_ids.clone())
        }

        fn poll_limit(&self) -> usize {
            self.poll_limit
        }

        async fn backoff_hs_should_skip(&self, hs_id: &str) -> bool {
            self.backoff.lock().await.should_skip(hs_id)
        }
    }

    #[tokio::test]
    async fn poll_limit_caps_the_homeservers_built() {
        let runner = RecordingRunner::new(&["hs1", "hs2", "hs3", "hs4"], 2);

        runner.run().await.unwrap();

        assert_eq!(*runner.built.lock().await, ["hs1", "hs2"]);
    }

    #[tokio::test]
    async fn backed_off_homeservers_do_not_use_up_the_poll_limit() {
        let runner = RecordingRunner::new(&["hs1", "hs2", "hs3", "hs4", "hs5"], 2);
        runner.backoff.lock().await.record_failure("hs1");
        runner.backoff.lock().await.record_failure("hs2");

        let ProcessedStats(stats) = runner.run().await.unwrap();

        // The two highest-priority HSs are in backoff, so the next two take their slots
        assert_eq!(*runner.built.lock().await, ["hs3", "hs4"]);
        assert_eq!(stats.count_skipped(), 2);
        assert_eq!(stats.count_failed_to_build(), 2);
    }
}
