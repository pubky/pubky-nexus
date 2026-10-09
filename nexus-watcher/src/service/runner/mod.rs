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

use nexus_common::models::user::{HomeserverReachability, HsReachability};
use nexus_common::types::DynError;
use nexus_common::StackManager;
use tokio::sync::watch::Receiver;
use tracing::{debug, error, info, warn};

use crate::service::{
    indexer::{RunError, TEventProcessor},
    stats::{ProcessedStats, ProcessorRunStatus, RunAllProcessorsStats},
};

/// What a run says about whether its homeserver answers: `Ok` after a successful run,
/// `Unreachable` when the run failed to reach it, `None` when the run says nothing about it
/// (a failure of Nexus's own, a panic, a timeout).
pub fn reachability_from_run_result(
    result: &Result<(), RunError>,
) -> Option<HomeserverReachability> {
    match result {
        Ok(_) => Some(HomeserverReachability::Ok),
        Err(RunError::Internal(e)) if e.is_homeserver_unreachable() => {
            Some(HomeserverReachability::Unreachable)
        }
        Err(_) => None,
    }
}

/// Records what a run observed of its homeserver, for light clients (#190). Light mode
/// only; a failed write is logged and does not fail the run.
async fn record_reachability(hs_id: &str, reachability: Option<HomeserverReachability>) {
    let Some(reachability) = reachability else {
        return;
    };
    if !StackManager::mode().is_light() {
        return;
    }
    if let Err(e) = HsReachability::record(hs_id, reachability).await {
        warn!(homeserver = %hs_id, error = %e, "Failed to record homeserver reachability");
    }
}

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
    /// Determines the list of target HS IDs to process in this run cycle.
    async fn pre_run(&self) -> Result<Vec<String>, DynError>;

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
        let mut run_stats = RunAllProcessorsStats::default();

        for hs_id in hs_ids {
            if *self.shutdown_rx().borrow() {
                info!(homeserver = %hs_id, "Shutdown detected; exiting run loop");
                break;
            }

            if self.backoff_hs_should_skip(&hs_id).await {
                debug!(homeserver = %hs_id, "Skipping homeserver in backoff");
                run_stats.add_run_result(hs_id, Duration::ZERO, ProcessorRunStatus::Skipped);
                continue;
            }

            let t0 = Instant::now();
            let (status, reachability) = match self.build(&hs_id).await {
                Ok(event_processor) => {
                    let result = event_processor.run().await;
                    let reachability = reachability_from_run_result(&result);
                    (status_from_run_result(result), reachability)
                }
                Err(e) => {
                    error!(homeserver = %hs_id, error = %e, "Failed to build event processor");
                    (ProcessorRunStatus::FailedToBuild, None)
                }
            };
            let duration = t0.elapsed();
            record_reachability(&hs_id, reachability).await;

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
    use super::{reachability_from_run_result, HomeserverReachability, RunError};
    use crate::EventProcessorError;
    use nexus_common::db::PubkyClientError;

    #[test]
    fn a_successful_run_means_the_homeserver_answers() {
        assert_eq!(
            reachability_from_run_result(&Ok(())),
            Some(HomeserverReachability::Ok)
        );
    }

    #[test]
    fn transport_failures_and_5xx_mean_unreachable() {
        let unreachable = [
            EventProcessorError::client_error("connection refused".into()),
            PubkyClientError::ServerError5xx {
                message: "502".into(),
            }
            .into(),
            EventProcessorError::HsEventsStreamTransportFailed("reset".into()),
        ];
        for error in unreachable {
            assert_eq!(
                reachability_from_run_result(&Err(RunError::Internal(error.clone()))),
                Some(HomeserverReachability::Unreachable),
                "{error}"
            );
        }
    }

    #[test]
    fn failures_of_nexus_own_say_nothing_about_the_homeserver() {
        let silent = [
            Err(RunError::Internal(EventProcessorError::GraphQueryFailed(
                true,
                "neo4j down".into(),
            ))),
            Err(RunError::Internal(
                EventProcessorError::IndexOperationFailed(true, "redis down".into()),
            )),
            Err(RunError::Internal(EventProcessorError::client_error_404(
                "gone".into(),
            ))),
            Err(RunError::Panicked),
            Err(RunError::TimedOut),
        ];
        for result in silent {
            assert_eq!(reachability_from_run_result(&result), None);
        }
    }
}
