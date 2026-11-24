//! Running processors across many homeservers.
//!
//! Use a [`TEventProcessorRunner`] when one application watches many homeservers: each
//! cycle it lists them, skips those in backoff, builds and runs one processor per
//! homeserver, one after the other, and reports how each turn ended in
//! [`RunAllProcessorsStats`].

use std::fmt::{Debug, Display};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::watch::Receiver;
use tracing::{debug, error, info};

use super::processor::{RunError, TEventProcessor};

/// How one homeserver's turn in a runner cycle ended.
/// Recorded in [`ProcessorRunStats::status`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessorRunStatus {
    /// [`TEventProcessorRunner::build`] failed, so no processor ran.
    FailedToBuild,
    /// The processor's run completed ([`status_from_run_result`] of `Ok`).
    Ok,
    /// The processor's run failed ([`RunError::Internal`]).
    Error,
    /// The processor's run panicked ([`RunError::Panicked`]).
    Panic,
    /// The processor's run stopped at its budget ([`RunError::BudgetSpent`]).
    BudgetSpent,
    /// [`TEventProcessorRunner::backoff_hs_should_skip`] skipped the homeserver.
    Skipped,
}

/// One homeserver's turn in a runner cycle.
/// Collected in [`RunAllProcessorsStats`].
pub struct ProcessorRunStats {
    /// The homeserver's ID, as [`TEventProcessorRunner::pre_run`] listed it.
    pub hs_id: String,
    /// How long building and running its processor took; zero when it was skipped.
    /// Measured by [`TEventProcessorRunner::run`].
    pub duration: Duration,
    /// How the turn ended, as a [`ProcessorRunStatus`].
    pub status: ProcessorRunStatus,
}

/// Every homeserver's turn in a runner cycle, in order.
/// Handed to [`TEventProcessorRunner::post_run`].
#[derive(Default)]
pub struct RunAllProcessorsStats {
    /// One [`ProcessorRunStats`] per homeserver, in the order they ran.
    pub stats: Vec<ProcessorRunStats>,
}

impl RunAllProcessorsStats {
    /// Records the turn of `hs_id`, which took `duration` and ended with `status`.
    /// Appends a [`ProcessorRunStats`].
    pub fn add_run_result(
        &mut self,
        hs_id: String,
        duration: Duration,
        status: ProcessorRunStatus,
    ) {
        let individual_run_stats = ProcessorRunStats {
            hs_id,
            duration,
            status,
        };
        self.stats.push(individual_run_stats);
    }

    fn count(&self, status: ProcessorRunStatus) -> usize {
        self.stats.iter().filter(|ps| ps.status == status).count()
    }

    /// Number of homeservers whose run completed ([`ProcessorRunStatus::Ok`]).
    pub fn count_ok(&self) -> usize {
        self.count(ProcessorRunStatus::Ok)
    }

    /// Number of homeservers whose run failed with an error ([`ProcessorRunStatus::Error`]).
    pub fn count_error(&self) -> usize {
        self.count(ProcessorRunStatus::Error)
    }

    /// Number of homeservers whose run panicked ([`ProcessorRunStatus::Panic`]).
    pub fn count_panic(&self) -> usize {
        self.count(ProcessorRunStatus::Panic)
    }

    /// Number of homeservers whose run stopped at its budget
    /// ([`ProcessorRunStatus::BudgetSpent`]).
    pub fn count_budget_spent(&self) -> usize {
        self.count(ProcessorRunStatus::BudgetSpent)
    }

    /// Number of homeservers whose processor could not be built
    /// ([`ProcessorRunStatus::FailedToBuild`]).
    pub fn count_failed_to_build(&self) -> usize {
        self.count(ProcessorRunStatus::FailedToBuild)
    }

    /// Number of homeservers skipped because they were in backoff
    /// ([`ProcessorRunStatus::Skipped`]).
    pub fn count_skipped(&self) -> usize {
        self.count(ProcessorRunStatus::Skipped)
    }
}

/// A cycle's [`RunAllProcessorsStats`] once [`TEventProcessorRunner::post_run`] has processed
/// them. [`TEventProcessorRunner::run`] returns it.
pub struct ProcessedStats(pub RunAllProcessorsStats);

/// The [`ProcessorRunStatus`] a processor run that ended with `result` is recorded with.
pub fn status_from_run_result<Err>(result: Result<(), RunError<Err>>) -> ProcessorRunStatus {
    match result {
        Ok(_) => ProcessorRunStatus::Ok,
        Err(RunError::Internal(_)) => ProcessorRunStatus::Error,
        Err(RunError::Panicked) => ProcessorRunStatus::Panic,
        Err(RunError::BudgetSpent) => ProcessorRunStatus::BudgetSpent,
    }
}

/// The orchestrator that builds and runs an event processor per homeserver, one after the
/// other, so that runs never overlap.
///
/// Generic over the processors' event type `E` and error type `Err`. Which homeservers run,
/// in what order, how many per cycle and which are skipped are hooks: [`Self::pre_run`],
/// [`Self::poll_limit`] and [`Self::backoff_hs_should_skip`].
///
/// # Implementation Notes
/// - The `build` method should create and return a fully configured event processor ready for immediate use
/// - Implementors should ensure that created processors are properly isolated and don't share mutable state unless explicitly intended
#[async_trait::async_trait]
pub trait TEventProcessorRunner<E, Err>: Send + Sync
where
    E: Send + Sync + 'static,
    Err: Display + Debug + Send + Sync + 'static,
{
    /// Why this runner could not list its homeservers ([`Self::pre_run`]) or build a
    /// processor ([`Self::build`]). These are the implementor's own lookups, so the
    /// implementor picks the type.
    type Error: Display + Debug + Send + Sync + 'static;

    /// The shutdown signal: once it is `true`, [`Self::run`] stops before the next homeserver.
    fn shutdown_rx(&self) -> Receiver<bool>;

    /// Creates and returns a new event processor instance for the specified homeserver.
    ///
    /// # Parameters
    /// * `hs_id` - The homeserver ID. Represents the homeserver this event processor will
    ///   fetch and process events from.
    ///
    /// # Returns
    /// A reference to the event processor instance, ready to be executed with its
    /// [`TEventProcessor::run`] method.
    ///
    /// # Errors
    /// Returns an error if the event processor couldn't be built.
    async fn build(&self, hs_id: &str) -> Result<Arc<dyn TEventProcessor<E, Err>>, Self::Error>;

    /// Pre-processing step before the main run loop.
    ///
    /// Determines the list of target HS IDs to process in this run cycle, ordered by priority.
    ///
    /// The list is not truncated here: [`Self::poll_limit`] is applied in [`Self::run`], after
    /// the HS IDs in backoff are skipped.
    async fn pre_run(&self) -> Result<Vec<String>, Self::Error>;

    /// Maximum number of HS IDs built and run per cycle. HS IDs skipped by
    /// [`Self::backoff_hs_should_skip`] don't count.
    ///
    /// No limit by default. Runners that cap the targets per run should overwrite this.
    fn poll_limit(&self) -> usize {
        usize::MAX
    }

    /// Post-processing of a cycle's [`RunAllProcessorsStats`], at the end of [`Self::run`].
    ///
    /// No-op default implementation. Callers that perform post-processing should overwrite this.
    async fn post_run(&self, stats: RunAllProcessorsStats) -> ProcessedStats {
        ProcessedStats(stats)
    }

    /// Main run loop: builds and runs event processors for the relevant targets.
    ///
    /// # Returns
    /// Statistics about the event processor run results, summarized as [`ProcessedStats`].
    async fn run(&self) -> Result<ProcessedStats, Self::Error> {
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

    /// Called before building a HS's processor, to check whether it should be skipped, e.g.
    /// because it is in backoff. A skipped HS does not count against [`Self::poll_limit`].
    ///
    /// No-op default implementation. Runners that use backoff should overwrite as needed.
    async fn backoff_hs_should_skip(&self, _hs_id: &str) -> bool {
        false
    }

    /// Called by [`Self::run`] after a HS is processed (build + run), with the
    /// [`ProcessorRunStatus`] of its turn, to update the backoff that
    /// [`Self::backoff_hs_should_skip`] checks.
    ///
    /// No-op default implementation. Runners that use backoff should overwrite as needed.
    async fn backoff_hs_record_result(&self, _hs_id: &str, _status: &ProcessorRunStatus) {}
}
