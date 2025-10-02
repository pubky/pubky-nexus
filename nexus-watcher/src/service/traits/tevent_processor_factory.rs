use std::sync::Arc;

use nexus_common::types::DynError;
use tokio::sync::watch::Receiver;
use tracing::{error, info};

use crate::service::{
    constants::MAX_HOMESERVERS_PER_RUN,
    traits::{tevent_processor::RunError, TEventProcessor},
};

#[derive(Default)]
pub struct RunAllProcessorsStats {
    /// Number of homeservers where processing were successful
    pub count_ok: u16,
    /// Number of homeservers where processing failed with Err
    pub count_error: u16,
    /// Number of homeservers where processing panicked
    pub count_panic: u16,
    /// Number of homeservers where processing timed out
    pub count_timeout: u16,
}

/// Wrapper around `RunAllProcessorsStats` which indicates they've been processed
pub struct ProcessedStats(pub RunAllProcessorsStats);

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
    async fn homeservers_by_priority(&self) -> Vec<String>;

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
    async fn pre_run_all(&self) -> Vec<String> {
        let hs_ids = self.homeservers_by_priority().await;
        let max = std::cmp::min(MAX_HOMESERVERS_PER_RUN, hs_ids.len());
        hs_ids[..max].to_vec()
    }

    /// Post-processing of the run results
    async fn post_run_all(&self, stats: RunAllProcessorsStats) -> ProcessedStats {
        info!(
            "Run result: {} ok, {} error, {} panic, {} timeout",
            stats.count_ok, stats.count_error, stats.count_panic, stats.count_timeout
        );
        ProcessedStats(stats)
    }

    /// Runs event processors for all homeservers relevant for this run, with timeout protection.
    ///
    /// # Returns
    /// Statistics about the event processor run results, summarized as [`RunAllProcessorsStats`]
    async fn run_all(&self) -> ProcessedStats {
        let hs_ids = self.pre_run_all().await;

        let mut run_stats = RunAllProcessorsStats::default();

        for hs_id in hs_ids {
            if *self.shutdown_rx().borrow() {
                info!("Shutdown detected in homeserver {hs_id}, exiting run_all loop");
                break; // Exit loop
            }

            let Ok(event_processor) = self.build(hs_id.clone()).await else {
                error!("Failed to build event processor for homeserver: {}", hs_id);
                continue; // Skip this loop iteration, continue with the next
            };

            match event_processor.run().await {
                Ok(_) => run_stats.count_ok += 1,
                Err(RunError::Internal(_)) => run_stats.count_error += 1,
                Err(RunError::Panicked) => run_stats.count_panic += 1,
                Err(RunError::TimedOut) => run_stats.count_timeout += 1,
            }
        }

        self.post_run_all(run_stats).await
    }
}
