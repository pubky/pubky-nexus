mod homeserver;
mod key_based;

pub use homeserver::HsEventProcessor;
pub use key_based::KeyBasedEventProcessor;
use nexus_common::models::event::ParseResult;
use std::{fmt::Display, path::PathBuf, sync::Arc, time::Duration};

use tracing::Instrument;

use nexus_common::models::event::{Event, EventProcessorError};
use tracing::{debug, error, warn};

use crate::events::retry::RetryScheduler;
use crate::events::EventHandler;
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

impl std::error::Error for RunError {}

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
    fn files_path(&self) -> &PathBuf;

    /// Returns the event handler used to process events.
    ///
    /// This allows for flexible event handling implementations, including mocked versions for testing.
    fn event_handler(&self) -> &Arc<dyn EventHandler>;

    /// Returns the instance name of the event processor, used in the monitoring and tracing spans.
    ///
    /// For instances mapped to a specific HS, this should include the HS ID.
    fn instance_name(&self) -> String;

    /// Returns the retry scheduler used by [`Self::handle_error`] to enqueue failed
    /// events for later retry.  Returns `None` when the processor bypasses
    /// [`Self::handle_error`] and manages retries on its own (e.g. [`RetryProcessor`](crate::events::retry::RetryProcessor)).
    fn retry_scheduler(&self) -> Option<&Arc<RetryScheduler>> {
        None
    }

    async fn run(self: Arc<Self>) -> Result<(), RunError> {
        let timeout = self
            .custom_timeout()
            .unwrap_or(Duration::from_secs(PROCESSING_TIMEOUT_SECS));

        let instance_name = self.instance_name();
        let span = tracing::info_span!("event_processor.run", service = %instance_name);
        let handle = tokio::spawn(self.run_internal().instrument(span));

        let join_result = tokio::time::timeout(timeout, handle)
            .await
            .inspect_err(|_| error!("Event processor timed out for {instance_name}"))
            .map_err(|_| RunError::TimedOut)?;

        // The JoinError can be:
        // - join_error.is_panic() => panic by the inner future
        // - join_error.is_cancelled() => inner future was abruptly interrupted, for example
        //   - JoinHandle::abort() is called on the handle
        //   - the Tokio runtime is shut down
        // In our model, we don't trigger such interruptions. Instead we use the shutdown signal
        // to gracefully stop the event processing loop. Therefore we consider all JoinErrors as panics.
        let run_internal_result = join_result
            .inspect_err(|je| error!("JoinError by event processor for {instance_name}: {je:?}"))
            .map_err(|_| RunError::Panicked)?;

        run_internal_result
            .inspect_err(|e| error!("Event processor failed for {instance_name}: {e:?}"))
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

    /// Parses a single event line and dispatches to [`Self::handle_event`].
    /// Unknown resource events are handled via `HomeserverParsedUri::UnknownResource` →
    /// `DefaultEventHandler` → `tag::sync_put_resource` (main flow).
    async fn process_event_line(&self, line: &str) -> Result<(), EventProcessorError> {
        match Event::parse_event(line, self.files_path().clone()) {
            Err(e) => error!("{e}"),
            Ok(ParseResult::Skipped) => {}
            Ok(ParseResult::UnrecognizedUri { reason, .. }) => {
                // Should not normally occur — UnknownResource parsing happens in HomeserverParsedUri
                warn!("Unrecognized event URI: {reason}");
            }
            Ok(ParseResult::Parsed(event)) => {
                debug!("Processing event: {:?}", event);
                self.handle_event(&event).await?;
            }
        }

        Ok(())
    }

    /// Handles an error of event processing from event processing (e.g. logging, scheduling retries).
    ///
    /// Called in the event processing loop.
    ///
    /// Returns:
    /// - `Ok(())` - Continue processing the batch (non-retryable errors are dropped, retryable
    ///   ones are queued for retry)
    /// - `Err(e)` - Stop processing and return error (for infrastructure errors)
    async fn handle_error(
        &self,
        event: &Event,
        error: EventProcessorError,
    ) -> Result<(), EventProcessorError> {
        if error.is_infrastructure() {
            warn!("Infrastructure error, stopping batch: {error}");
            return Err(error);
        }

        if !error.is_retryable() {
            debug!("Non-retryable error, skipping event {}: {error}", event.uri);
            return Ok(());
        }

        let Some(scheduler) = self.retry_scheduler() else {
            return Ok(());
        };

        if error.is_missing_dependency() {
            scheduler.queue_missing_dep(event).await
        } else {
            warn!("Retryable error, queuing event for retry: {error}");
            scheduler.queue_transient(event).await
        }
    }

    /// Processes an event and delegates to [`Self::handle_error`] on failure.
    #[tracing::instrument(
        name = "event.process",
        skip_all,
        fields(
            event.resource = %event.parsed_uri.resource(),
            event.uri = %event.uri,
            event.r#type = %event.event_type,
            event.user_id = %event.parsed_uri.user_id(),
            event.resource_id = event.parsed_uri.resource().id().unwrap_or_default(),
            instance = %self.instance_name(),
            otel.status_code = tracing::field::Empty,
            otel.status_message = tracing::field::Empty,
        )
    )]
    async fn handle_event(&self, event: &Event) -> Result<(), EventProcessorError> {
        let span = tracing::Span::current();
        if let Err(e) = self.event_handler().handle(event).await {
            span.record("otel.status_code", "ERROR");
            span.record("otel.status_message", tracing::field::display(&e));

            self.handle_error(event, e).await?;
        } else {
            span.record("otel.status_code", "OK");
        }

        Ok(())
    }
}
