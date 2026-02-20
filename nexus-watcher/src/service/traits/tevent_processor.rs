use std::{fmt::Display, sync::Arc, time::Duration};

use nexus_common::models::event::EventProcessorError;
use pubky_app_specs::PubkyId;
use tracing::error;

use crate::service::PROCESSING_TIMEOUT_SECS;

/// Possible error types of an event processor run
#[derive(Debug)]
pub enum RunError {
    Internal(EventProcessorError),
    Panicked,
    TimedOut,
}

impl RunError {
    pub fn is_panic(&self) -> bool {
        matches!(self, RunError::Panicked)
    }

    pub fn is_timeout(&self) -> bool {
        matches!(self, RunError::TimedOut)
    }
}

impl Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::Internal(err) => write!(f, "Internal error: {err}"),
            RunError::Panicked => write!(f, "Execution panicked"),
            RunError::TimedOut => write!(f, "Execution timed out"),
        }
    }
}

/// Asynchronous event processor interface for the Watcher service.
///
/// This trait represents a component that can process events asynchronously and can be
/// gracefully shut down through a watch channel.
///
/// # Implementation Notes
/// - Implementors should regularly check the `shutdown_rx` channel for shutdown signals
///   and terminate gracefully when received
/// - The method returns an `EventProcessorError` to allow for typed error handling across
///   different processor implementations
#[async_trait::async_trait]
pub trait TEventProcessor: Send + Sync + 'static {
    fn get_homeserver_id(&self) -> PubkyId;

    async fn run(self: Arc<Self>) -> Result<(), RunError> {
        let hs_id = self.get_homeserver_id().to_string();
        let timeout = self
            .custom_timeout()
            .unwrap_or(Duration::from_secs(PROCESSING_TIMEOUT_SECS));

        let handle = tokio::spawn(self.run_internal());

        let join_result = tokio::time::timeout(timeout, handle)
            .await
            .inspect_err(|_| error!("Event processor timed out for {hs_id}"))
            .map_err(|_| RunError::TimedOut)?;

        // The JoinError can be:
        // - join_error.is_panic() => panic by the inner future
        // - join_error.is_cancelled() => inner future was abruptly interrupted, for example
        //   - JoinHandle::abort() is called on the handle
        //   - the Tokio runtime is shut down
        // In our model, we don't trigger such interruptions. Instead we use the shutdown signal
        // to gracefully stop the event processing loop. Therefore we consider all JoinErrors as panics.
        let run_internal_result = join_result
            .inspect_err(|je| error!("JoinError while running event processor for {hs_id}: {je:?}"))
            .map_err(|_| RunError::Panicked)?;

        run_internal_result
            .inspect_err(|e| error!("Event processor failed for {hs_id}: {e:?}"))
            .map_err(RunError::Internal)
    }

    /// Runs the event processor asynchronously.
    ///
    /// Returns `Ok(())` on a clean exit, or `Err(EventProcessorError)` on failure.
    async fn run_internal(self: Arc<Self>) -> Result<(), EventProcessorError>;

    /// Optional custom timeout for this event processor.
    ///
    /// If not set, the [`PROCESSING_TIMEOUT_SECS`] is applied.
    fn custom_timeout(&self) -> Option<Duration> {
        None
    }
}
