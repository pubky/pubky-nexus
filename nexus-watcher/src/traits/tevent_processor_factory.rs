use nexus_common::types::DynError;
use crate::traits::TEventProcessor;

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
    /// The returned processor is fully configured and ready to be executed with its
    /// `run` method. Ownership of the processor is transferred to the caller.
    async fn build(&self, homeserver_id: String) -> Result<Box<dyn TEventProcessor>, DynError>;
}