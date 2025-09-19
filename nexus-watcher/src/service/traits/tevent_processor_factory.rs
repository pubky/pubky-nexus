use std::sync::Arc;

use nexus_common::types::DynError;
use tokio::sync::watch::Receiver;
use tracing::{error, info};

use crate::service::traits::{tevent_processor::RunError, TEventProcessor};

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

/// The type that describes the result of an event processor run
type RunAllProcessorsResult = Result<RunAllProcessorsStats, DynError>;

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
    /// * `homeserver_id` - The homeserver identifier (must be a valid `PubkyId`
    ///   string). Used to configure the processor with homeserver-specific settings
    ///   and connections.
    ///
    /// # Returns
    /// Returns `Ok(Box<dyn TEventProcessor>)` containing the newly created processor
    /// instance on success, or `Err(DynError)` if processor creation fails.
    ///
    /// The returned processor is fully configured and ready to be executed with its `run` method.
    async fn build(&self, homeserver_id: String) -> Result<Arc<dyn TEventProcessor>, DynError>;

    /// Runs event processors for all homeservers retrieved from the graph.
    ///
    /// This method iterates through all homeserver IDs relevant for this run,
    /// creates an event processor for each one, and executes them with timeout protection.
    /// It tracks both successfully processed homeservers and those that failed.
    ///
    /// # Returns
    /// Returns `Ok((count_ok, count_error))` where:
    /// - `count_ok`: Number of homeservers where processing returned Ok
    /// - `count_error`: Number of homeservers where processing failed with Err
    async fn run_all(&self) -> RunAllProcessorsResult {
        let hs_ids = self.homeservers_by_priority().await;

        let mut run_stats = RunAllProcessorsStats::default();

        for hs_id in hs_ids {
            if *self.shutdown_rx().borrow() {
                info!("Shutdown detected in homeserver {hs_id}, exiting run_all loop");
                return Ok(run_stats);
            }

            let Ok(event_processor) = self.build(hs_id.clone()).await else {
                error!("Failed to build event processor for homeserver: {}", hs_id);
                continue;
            };

            match event_processor.run().await {
                Ok(_) => run_stats.count_ok += 1,
                Err(RunError::Internal(_)) => run_stats.count_error += 1,
                Err(RunError::Panicked) => run_stats.count_panic += 1,
                Err(RunError::TimedOut) => run_stats.count_timeout += 1,
            }
        }

        Ok(run_stats)
    }
}
