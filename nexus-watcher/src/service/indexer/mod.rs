mod homeserver;
mod key_based;

pub use homeserver::HsEventProcessor;
pub use key_based::{KeyBasedEventProcessor, KeyBasedEventSource, PubkyKeyBasedEventSource};

use std::{fmt::Display, sync::Arc, time::Duration};

use crate::errors::EventProcessorError;
use crate::events::retry::RetryScheduler;
use crate::events::EventHandler;
use crate::events::{Event, ParseResult};
use crate::service::SLOW_RUN_THRESHOLD;
use tracing::{debug, error, trace, warn, Instrument};

/// OpenTelemetry meter name shared by all watcher indexer metrics.
pub(super) const METER_NAME: &str = "nexus.watcher";

/// Possible error types of an event processor run
#[derive(Debug)]
pub enum RunError {
    Internal(EventProcessorError),
    Panicked,
    /// The run stopped at its budget, see [`EventProcessorError::RunBudgetSpent`]
    BudgetSpent,
}

impl RunError {
    pub fn is_panic(&self) -> bool {
        matches!(self, RunError::Panicked)
    }
}

impl std::error::Error for RunError {}

impl Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::Internal(err) => write!(f, "Internal error: {err}"),
            RunError::Panicked => write!(f, "Execution panicked"),
            RunError::BudgetSpent => write!(f, "Run budget spent"),
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
    /// Returns the event handler used to process events.
    ///
    /// This allows for flexible event handling implementations, including mocked versions for testing.
    fn event_handler(&self) -> &Arc<dyn EventHandler>;

    /// Returns a short service label for monitoring and tracing spans (e.g. `HsEventProcessor`).
    fn instance_name(&self) -> &'static str;

    /// Returns the retry scheduler used by [`Self::handle_error`] to enqueue failed
    /// events for later retry.  Returns `None` when the processor bypasses
    /// [`Self::handle_error`] and manages retries on its own (e.g. [`RetryProcessor`](crate::events::retry::RetryProcessor)).
    fn retry_scheduler(&self) -> Option<&Arc<RetryScheduler>> {
        None
    }

    fn homeserver_id(&self) -> Option<&str> {
        None
    }

    /// Runs [`Self::run_internal`] on its own task, and waits for it to end.
    ///
    /// A run that takes longer than [`Self::slow_run_threshold`] is logged as slow, again after
    /// each further threshold, and still waited for: it is not stopped, so the next run cannot
    /// start next to it. A run that never returns therefore holds up its loop for good.
    async fn run(self: Arc<Self>) -> Result<(), RunError> {
        let slow_run_threshold = self.slow_run_threshold().unwrap_or(SLOW_RUN_THRESHOLD);

        let instance_name = self.instance_name();
        let homeserver_id = self.homeserver_id().map(str::to_owned);
        let homeserver = homeserver_id.as_deref().map(tracing::field::display);
        let span = tracing::info_span!(
            "event_processor.run",
            service = %instance_name,
            homeserver,
        );
        let started = tokio::time::Instant::now();
        let mut handle = tokio::spawn(self.run_internal().instrument(span));

        let join_result = loop {
            match tokio::time::timeout(slow_run_threshold, &mut handle).await {
                Ok(join_result) => break join_result,
                Err(_) => warn!(
                    service = %instance_name,
                    homeserver,
                    running_for_secs = started.elapsed().as_secs(),
                    "Event processor run is slow; waiting for it to finish"
                ),
            }
        };

        // The JoinError can be:
        // - join_error.is_panic() => panic by the inner future
        // - join_error.is_cancelled() => inner future was abruptly interrupted, for example
        //   - JoinHandle::abort() is called on the handle
        //   - the Tokio runtime is shut down
        // In our model, we don't trigger such interruptions. Instead we use the shutdown signal
        // to gracefully stop the event processing loop. Therefore we consider all JoinErrors as panics.
        let run_internal_result = join_result
            .inspect_err(|je| {
                error!(
                    service = %instance_name,
                    homeserver,
                    error = ?je,
                    "Event processor JoinError"
                )
            })
            .map_err(|_| RunError::Panicked)?;

        run_internal_result.map_err(|e| match e {
            EventProcessorError::RunBudgetSpent => RunError::BudgetSpent,
            e => {
                error!(
                    service = %instance_name,
                    homeserver,
                    error = ?e,
                    "Event processor failed"
                );
                RunError::Internal(e)
            }
        })
    }

    /// Runs the event processor asynchronously.
    ///
    /// Returns `Ok(())` on a clean exit, or `Err(EventProcessorError)` on failure.
    async fn run_internal(self: Arc<Self>) -> Result<(), EventProcessorError>;

    /// How long a run of this event processor may take before it is logged as slow.
    ///
    /// If not set, the [`SLOW_RUN_THRESHOLD`] is applied.
    fn slow_run_threshold(&self) -> Option<Duration> {
        None
    }

    /// Parses a single event line and dispatches to [`Self::handle_event`].
    /// Universal tag events are handled via `ExtendedParsedUri::UniversalTag` →
    /// `DefaultEventHandler` → `tag::sync_put_resource`.
    async fn process_event_line(&self, line: &str) -> Result<(), EventProcessorError> {
        match Event::parse_event(line) {
            // Invalid event lines come from untrusted homeservers; treat as bad peer data, not Nexus errors.
            Err(e) => warn!(error = %e, "Invalid event line"),
            Ok(ParseResult::Skipped) => {}
            Ok(ParseResult::UnrecognizedUri { reason, .. }) => {
                warn!(%reason, "Unrecognized event URI");
            }
            Ok(ParseResult::Parsed(event)) => {
                trace!(?event, "Processing event");
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
    /// - `Ok(())` - Continue processing the batch (errors not worth retrying are dropped, the
    ///   rest are queued for retry)
    /// - `Err(e)` - Stop processing and return error (for errors that should not be retried right now)
    async fn handle_error(
        &self,
        event: &Event,
        error: EventProcessorError,
    ) -> Result<(), EventProcessorError> {
        if error.should_not_retry_now() {
            warn!(error = %error, "Got should-not-retry-now error, stopping batch");
            return Err(error);
        }

        if !RetryScheduler::should_enqueue_related_event(&error) {
            debug!(
                event.uri = %event.uri,
                error = %error,
                "Error not worth retrying, skipping event"
            );
            return Ok(());
        }

        let Some(scheduler) = self.retry_scheduler() else {
            return Ok(());
        };

        let Some(homeserver_id) = self.homeserver_id() else {
            warn!(
                event.uri = %event.uri,
                "Retryable error but no origin homeserver to persist; skipping retry"
            );
            return Ok(());
        };

        if error.is_missing_dependency() {
            scheduler.queue_missing_dep(event, homeserver_id).await
        } else {
            warn!(error = %error, "Transient error, queuing event for retry");
            scheduler.queue_transient(event, homeserver_id).await
        }
    }

    /// Decides whether the given event should be processed by this processor.
    ///
    /// Called at the start of [`Self::handle_event`], before the event is handed
    /// to the event handler. The default always processes the event;
    /// [`HsEventProcessor`](crate::service::indexer::HsEventProcessor) overrides
    /// this to skip events from users bound to a different homeserver.
    async fn should_process_event(&self, _event: &Event) -> Result<bool, EventProcessorError> {
        Ok(true)
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
            otel.status_code = tracing::field::Empty,
            otel.status_message = tracing::field::Empty,
        )
    )]
    async fn handle_event(&self, event: &Event) -> Result<(), EventProcessorError> {
        let span = tracing::Span::current();

        match self.should_process_event(event).await {
            Ok(true) => {}
            Ok(false) => {
                span.record("otel.status_code", "UNSET");
                span.record("otel.status_message", "SKIPPED");
                return Ok(());
            }
            Err(e) => {
                span.record("otel.status_code", "ERROR");
                span.record("otel.status_message", tracing::field::display(&e));
                return self.handle_error(event, e).await;
            }
        }

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
