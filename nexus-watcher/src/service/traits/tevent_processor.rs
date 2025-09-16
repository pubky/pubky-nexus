use std::{fmt::Display, sync::Arc, time::Duration};

use nexus_common::types::DynError;
use pubky_app_specs::PubkyId;
use tracing::error;

use crate::service::PROCESSING_TIMEOUT_SECS;

/// Possible error types of an event processor run
#[derive(Debug)]
pub enum RunError {
    Internal(DynError),
    Panicked,
    TimedOut,
}

impl RunError {
    pub fn is_panic(&self) -> bool {
        matches!(self, RunError::Panicked)
    }
}

impl Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
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
/// - The method returns a `DynError` to allow for flexible error handling across
///   different processor implementations
#[async_trait::async_trait]
pub trait TEventProcessor: Send + Sync + 'static {
    fn get_homeserver_id(&self) -> PubkyId;

    async fn run(self: Arc<Self>) -> Result<(), RunError> {
        let hs_id = self.get_homeserver_id().to_string();
        let timeout = self
            .timeout()
            .unwrap_or(Duration::from_secs(PROCESSING_TIMEOUT_SECS));

        let handle = tokio::spawn(self.run_internal());

        let join_result = tokio::time::timeout(timeout, handle)
            .await
            .inspect_err(|_| error!("Event processor timed out for {hs_id}"))
            .map_err(|_| RunError::TimedOut)?;

        match join_result {
            Ok(run_internal_result) => match run_internal_result {
                Ok(_) => Ok(()),
                Err(e) => {
                    error!("Event processor failed for {hs_id}: {e:?}");
                    Err(RunError::Internal(e))
                }
            },

            Err(join_error) => {
                // The JoinError can be:
                // - join_error.is_panic() => panic by the inner future
                // - join_error.is_cancelled() => inner future was abruptly interrupted, for example
                //   - JoinHandle::abort() is called on the handle
                //   - the Tokio runtime is shut down

                error!("JoinError while running event processor for {hs_id}: {join_error:?}");
                Err(RunError::Panicked)
            }
        }
    }

    /// Runs the event processor asynchronously.
    ///
    /// Returns `Ok(())` on a clean exit, or `Err(DynError)` on failure.
    async fn run_internal(self: Arc<Self>) -> Result<(), DynError>;

    /// Optional custom timeout for this event processor execution.
    ///
    /// If not set, the [`PROCESSING_TIMEOUT_SECS`] is applied.
    fn timeout(&self) -> Option<Duration> {
        None
    }
}
