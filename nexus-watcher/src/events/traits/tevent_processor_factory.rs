use std::{sync::Arc, time::Duration};

use crate::events::TEventProcessor;
use nexus_common::{models::homeserver::Homeserver, types::DynError};
use tokio::{sync::watch::Receiver, time::timeout};
use tracing::{error, info};

/// The type that describes the result of an event processor run
type RunAllProcessorsResult = Result<(u64, u64), DynError>;

/// Asynchronous factory for creating event processors in the Watcher service.
///
/// This trait represents a component responsible for creating event processor instances
/// for specific homeservers. It provides a standardized way to instantiate processors
/// with the appropriate configuration and dependencies.
///
/// # Thread Safety
/// Implementors must be `Send + Sync` to ensure they can be safely used across thread
/// boundaries, which is essential for asynchronous factory operations.
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
pub trait TEventProcessorFactory: Send + Sync {
    /// Returns the timeout duration for event processor execution.
    ///
    /// This timeout is applied to individual event processor `run()` operations
    /// to prevent hanging or long-running processors from blocking the system
    fn timeout(&self) -> Duration;

    fn shutdown_rx(&self) -> Receiver<bool>;

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
    /// This method iterates through all homeserver IDs stored in the graph database,
    /// creates an event processor for each one, and executes them with timeout protection.
    /// It tracks both successfully processed homeservers and those that were skipped
    ///
    /// # Returns
    /// Returns `Ok((count_ok, count_error))` where:
    /// - `count_ok`: Number of homeservers where processing returned Ok
    /// - `count_error`: Number of homeservers where processing failed with Err
    async fn run_all(&self) -> RunAllProcessorsResult {
        let hs_ids = Homeserver::get_all_from_graph()
            .await
            .expect("No Homeserver IDs found in graph");

        let mut count_ok = 0;
        let mut count_error = 0;

        for hs_id in hs_ids {
            if *self.shutdown_rx().borrow() {
                info!("Shutdown detected in homeserver {hs_id}, exiting run_all loop");
                return Ok((count_ok, count_error));
            }

            let Ok(event_processor) = self.build(hs_id.clone()).await else {
                error!("Failed to build event processor for homeserver: {}", hs_id);
                continue;
            };
            match timeout(self.timeout(), event_processor.run()).await {
                Ok(Ok(_)) => count_ok += 1,
                Ok(Err(e)) => {
                    error!("Event processor failed for {}: {:?}", hs_id, e);
                    count_error += 1;
                }
                Err(_) => {
                    error!("Event processor timed out for {}", hs_id);
                    count_error += 1;
                }
            }
        }

        Ok((count_ok, count_error))
    }

    /// Runs an event processor for a specific homeserver.
    ///
    /// This method creates an event processor for the specified homeserver ID and
    /// executes it with timeout protection
    ///
    /// # Parameters
    /// * `hs_id` - The homeserver identifier as a string. Must be a valid PubkyId
    ///   that exists in the system and can be processed by the factory.
    ///
    /// # Returns
    /// Returns `Ok(())` if the processor completes successfully within the timeout,
    /// or `Err(DynError)` if:
    /// - The processor cannot be built for the given homeserver
    /// - The processor fails during execution
    /// - The processor times out
    /// - A shutdown is requested (treated as an error in single-run mode)
    async fn run(&self, hs_id: String) -> Result<(), DynError> {
        let Ok(event_processor) = self.build(hs_id.clone()).await else {
            error!("Failed to build event processor for homeserver: {}", hs_id);
            return Err(DynError::from(format!(
                "Failed to build event processor for homeserver: {}",
                hs_id
            )));
        };
        match timeout(self.timeout(), event_processor.run()).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => {
                error!("Event processor failed for {}: {:?}", hs_id, e);
                return Err(DynError::from(format!(
                    "Event processor failed for {}: {:?}",
                    hs_id, e
                )));
            }
            Err(_) => {
                error!("Event processor timed out for {}", hs_id);
                return Err(DynError::from(format!(
                    "Event processor timed out for {}",
                    hs_id
                )));
            }
        }
    }
}
