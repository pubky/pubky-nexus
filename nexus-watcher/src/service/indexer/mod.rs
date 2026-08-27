mod homeserver;
mod key_based;
mod processor_run;

pub use homeserver::HsEventProcessor;
pub use key_based::{KeyBasedEventProcessor, KeyBasedEventSource, PubkyKeyBasedEventSource};
pub use processor_run::{ProcessorResult, RunCompletion, RunContext, RunError, TimeoutPolicy};

use std::{sync::Arc, time::Duration};
use tokio::task::JoinHandle;

use crate::errors::EventProcessorError;
use crate::events::retry::RetryScheduler;
use crate::events::EventHandler;
use crate::events::{Event, ParseResult};
use crate::service::PROCESSING_TIMEOUT_SECS;
use tracing::{debug, error, trace, warn, Instrument};

/// OpenTelemetry meter name shared by all watcher indexer metrics.
pub(super) const METER_NAME: &str = "nexus.watcher";

fn resolve_processor_task_result(
    result: Result<ProcessorResult, tokio::task::JoinError>,
    instance_name: &str,
    homeserver_id: Option<&str>,
) -> Result<RunCompletion, RunError> {
    let homeserver = homeserver_id.map(tracing::field::display);
    match result {
        Ok(Ok(completion)) => Ok(completion),
        Ok(Err(error)) => {
            error!(
                service = %instance_name,
                homeserver,
                ?error,
                "Event processor failed"
            );
            Err(RunError::Internal(error))
        }
        Err(error) => {
            error!(
                service = %instance_name,
                homeserver,
                ?error,
                "Event processor task failed"
            );
            Err(RunError::Panicked)
        }
    }
}

async fn force_abort_processor_task(
    handle: &mut JoinHandle<ProcessorResult>,
) -> Result<RunCompletion, RunError> {
    handle.abort();
    // TODO: Bound this wait and fence unfinished tasks so later processor runs cannot overlap.
    let _ = handle.await;
    Err(RunError::TimedOut)
}

/// Asynchronous event processor interface for the Watcher service.
///
/// Processors support watcher-wide shutdown and may also observe a run-scoped
/// budget through [`RunContext`].
///
/// # Implementation Notes
/// - Check watcher shutdown at safe processing boundaries.
/// - Processors using cooperative timeouts should also check [`RunContext::is_budget_exhausted`].
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

    async fn run(self: Arc<Self>) -> Result<RunCompletion, RunError> {
        let timeout = self
            .custom_timeout()
            .unwrap_or(Duration::from_secs(PROCESSING_TIMEOUT_SECS));

        let instance_name = self.instance_name();
        let homeserver_id = self.homeserver_id().map(str::to_owned);
        let homeserver = homeserver_id.as_deref().map(tracing::field::display);
        let timeout_policy = self.timeout_policy();

        let span = tracing::info_span!(
            "event_processor.run",
            service = %instance_name,
            homeserver,
        );

        let (budget_exhaustion_tx, context) = RunContext::with_budget_signal();
        let mut handle = tokio::spawn(self.run_internal(context).instrument(span));

        match tokio::time::timeout(timeout, &mut handle).await {
            Ok(join_result) => {
                resolve_processor_task_result(join_result, instance_name, homeserver_id.as_deref())
            }
            Err(_) => match timeout_policy {
                TimeoutPolicy::HardAbort => {
                    error!(service = %instance_name, homeserver, "Event processor timed out");
                    force_abort_processor_task(&mut handle).await
                }
                TimeoutPolicy::Cooperative { grace } => {
                    debug!(
                        service = %instance_name,
                        homeserver,
                        ?grace,
                        "Event processor budget exhausted; requesting cooperative stop"
                    );
                    budget_exhaustion_tx.send_replace(true);

                    match tokio::time::timeout(grace, &mut handle).await {
                        Ok(join_result) => resolve_processor_task_result(
                            join_result,
                            instance_name,
                            homeserver_id.as_deref(),
                        ),
                        Err(_) => {
                            error!(
                                service = %instance_name,
                                homeserver,
                                ?grace,
                                "Event processor did not stop within grace period"
                            );
                            force_abort_processor_task(&mut handle).await
                        }
                    }
                }
            },
        }
    }

    /// Runs the event processor asynchronously.
    ///
    /// Returns the completion reason on a clean exit, or an error on failure.
    async fn run_internal(self: Arc<Self>, context: RunContext) -> ProcessorResult;

    /// Selects immediate hard abort or cooperative cancellation after a timeout.
    fn timeout_policy(&self) -> TimeoutPolicy {
        TimeoutPolicy::HardAbort
    }

    /// Optional custom timeout for this event processor.
    ///
    /// If not set, the [`PROCESSING_TIMEOUT_SECS`] is applied.
    fn custom_timeout(&self) -> Option<Duration> {
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
