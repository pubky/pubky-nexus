use std::{sync::Arc, time::Instant};

use nexus_common::types::DynError;
use tokio::sync::watch::Receiver;
use tracing::{error, info};

use crate::service::{
    constants::MAX_HOMESERVERS_PER_RUN,
    stats::{ProcessedStats, ProcessorRunStatus, RunAllProcessorsStats},
    traits::{tevent_processor::RunError, TEventProcessor},
};

/// Asynchronous factory for creating event processors in the Watcher service.
///
/// This trait represents a component responsible for creating event processor instances
/// for specific homeservers. It provides a standardized way to instantiate processors
/// with the appropriate configuration and dependencies.
///
/// # Implementation Notes
/// - The `build` method should create and return a fully configured event processor
///   ready for immediate use
/// - Factory implementations should initialize dependencies and configuration
///   for the created processors
/// - The method returns a `Result` to allow for proper error handling during processor
///   creation, avoiding panics in production code
/// - Implementors should ensure that created processors are properly isolated and
///   don't share mutable state unless explicitly intended
#[async_trait::async_trait]
pub trait TEventProcessorFactory {
    /// Returns the shutdown signal receiver
    fn shutdown_rx(&self) -> Receiver<bool>;

    /// Returns the default homeserver ID for this factory.
    /// This is used to prioritize the default homeserver when processing multiple homeservers.
    fn default_homeserver(&self) -> &str;

    /// Returns the homeserver IDs relevant for this run, ordered by their priority.
    ///
    /// Contains all homeserver IDs from the graph, with the default homeserver prioritized at index 0.
    async fn homeservers_by_priority(&self) -> Result<Vec<String>, DynError>;

    /// Creates and returns a new event processor instance for the specified homeserver.
    ///
    /// # Parameters
    /// * `homeserver_id` - The homeserver PubkyId. Represents the homeserver this event processor will
    /// fetch and process events from.
    ///
    /// # Returns
    /// A reference to the event processor instance, ready to be executed with its `run` method.
    ///
    /// # Errors
    /// Throws a [`DynError`] if the event processor couldn't be built
    async fn build(&self, homeserver_id: String) -> Result<Arc<dyn TEventProcessor>, DynError>;

    /// Decides the homeservers (which ones and in which order) from which events will be fetched and processed.
    ///
    /// # Returns
    /// Ordered list of homeserver IDs considered for `run_all`, from highest to lowest prio.
    async fn pre_run_all(&self) -> Result<Vec<String>, DynError> {
        let hs_ids = self.homeservers_by_priority().await?;
        let max = std::cmp::min(MAX_HOMESERVERS_PER_RUN, hs_ids.len());
        Ok(hs_ids[..max].to_vec())
    }

    /// Post-processing of the run results
    async fn post_run_all(&self, stats: RunAllProcessorsStats) -> ProcessedStats {
        for individual_run_stat in &stats.stats {
            let hs_id = &individual_run_stat.hs_id;
            let duration = individual_run_stat.duration;
            let status = &individual_run_stat.status;
            info!("Event processor run for HS {hs_id}: duration {duration:?}, status {status:?}");
        }

        let count_ok = stats.count_ok();
        let count_error = stats.count_error();
        let count_panic = stats.count_panic();
        let count_timeout = stats.count_timeout();
        info!("Run result: {count_ok} ok, {count_error} error, {count_panic} panic, {count_timeout} timeout");

        ProcessedStats(stats)
    }

    /// Runs event processors for all homeservers relevant for this run, with timeout protection.
    ///
    /// # Returns
    /// Statistics about the event processor run results, summarized as [`RunAllProcessorsStats`]
    async fn run_all(&self) -> Result<ProcessedStats, DynError> {
        let hs_ids = self.pre_run_all().await?;

        let mut run_stats = RunAllProcessorsStats::default();

        for hs_id in hs_ids {
            if *self.shutdown_rx().borrow() {
                info!("Shutdown detected in homeserver {hs_id}, exiting run_all loop");
                break; // Exit loop
            }

            let t0 = Instant::now();
            let status = match self.build(hs_id.clone()).await {
                Ok(event_processor) => match event_processor.run().await {
                    Ok(_) => ProcessorRunStatus::Ok,
                    Err(RunError::Internal(_)) => ProcessorRunStatus::Error,
                    Err(RunError::Panicked) => ProcessorRunStatus::Panic,
                    Err(RunError::TimedOut) => ProcessorRunStatus::Timeout,
                },
                Err(e) => {
                    error!("Failed to build event processor for homeserver: {hs_id}: {e}");
                    ProcessorRunStatus::FailedToBuild
                }
            };
            let duration = Instant::now().duration_since(t0);

            run_stats.add_run_result(hs_id, duration, status);
        }

        let processed_stats = self.post_run_all(run_stats).await;
        Ok(processed_stats)
    }
}
