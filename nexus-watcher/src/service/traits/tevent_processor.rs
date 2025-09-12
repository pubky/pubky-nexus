use std::sync::Arc;

use nexus_common::types::DynError;

/// Asynchronous event processor interface for the Watcher service.
///
/// This trait represents a component that can process events asynchronously and can be
/// gracefully shut down through a watch channel.
///
/// # Thread Safety
/// Implementors must be `Send + Sync` to ensure they can be safely used across thread
/// boundaries, which is crucial for asynchronous event processing.
///
/// # Implementation Notes
/// - Implementors should regularly check the `shutdown_rx` channel for shutdown signals
///   and terminate gracefully when received
/// - The method returns a `DynError` to allow for flexible error handling across
///   different processor implementations
#[async_trait::async_trait]
pub trait TEventProcessor: Send + Sync {
    /// Runs the event processor asynchronously.
    ///
    /// Returns `Ok(())` on a clean exit, or `Err(DynError)` on failure.
    async fn run(self: Arc<Self>) -> Result<(), DynError>;
}
