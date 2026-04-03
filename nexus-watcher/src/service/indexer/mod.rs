mod homeserver;
mod key_based;

pub use homeserver::HsEventProcessor;
pub use key_based::KeyBasedEventProcessor;
use std::{fmt::Display, path::PathBuf, sync::Arc, time::Duration};
use tracing::Instrument;

use nexus_common::models::event::{Event, EventProcessorError};
use tracing::{debug, error};

use crate::events::handle;
use crate::events::Moderation;
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
    fn files_path(&self) -> &PathBuf;
    fn moderation(&self) -> &Arc<Moderation>;

    async fn run(self: Arc<Self>) -> Result<(), RunError> {
        let timeout = self
            .custom_timeout()
            .unwrap_or(Duration::from_secs(PROCESSING_TIMEOUT_SECS));

        // Extract the class name of this instance
        let instance_type_name = std::any::type_name::<Self>();
        // TODO X1: Ensure we are using correct instance name (incl. which HS ID is used in case of HS Runner, etc)
        let span = tracing::info_span!("event_processor.run", service = %instance_type_name);
        let handle = tokio::spawn(self.run_internal().instrument(span));

        let join_result = tokio::time::timeout(timeout, handle)
            .await
            .inspect_err(|_| error!("Event processor timed out for {instance_type_name}")) // TODO See X1
            .map_err(|_| RunError::TimedOut)?;

        // The JoinError can be:
        // - join_error.is_panic() => panic by the inner future
        // - join_error.is_cancelled() => inner future was abruptly interrupted, for example
        //   - JoinHandle::abort() is called on the handle
        //   - the Tokio runtime is shut down
        // In our model, we don't trigger such interruptions. Instead we use the shutdown signal
        // to gracefully stop the event processing loop. Therefore we consider all JoinErrors as panics.
        let run_internal_result = join_result
            .inspect_err(|je| {
                error!("JoinError while running event processor for {instance_type_name}: {je:?}")
                // TODO See X1
            })
            .map_err(|_| RunError::Panicked)?;

        run_internal_result
            .inspect_err(|e| error!("Event processor failed for {instance_type_name}: {e:?}")) // TODO See X1
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

    /// Parses a single event line, creates a tracing span, and dispatches to [`Self::handle_event`].
    async fn process_event_line(&self, line: &str) -> Result<(), EventProcessorError> {
        let maybe_event = Event::parse_event(line, self.files_path().clone())
            .inspect_err(|e| error!("{e}"))
            .unwrap_or(None);

        if let Some(event) = maybe_event {
            debug!("Processing event: {:?}", event);
            self.handle_event(&event).await?;
        }

        Ok(())
    }

    /// Handles an error of event processing from event processing (e.g. logging, scheduling retries).
    ///
    /// Called in the event processing loop.
    ///
    /// It re-throws the erorr, which signals the caller should break the processing loop
    /// and persist the cursor at the point where this error occurred. This is because, since
    /// it's an infrastructure error, it is likely that trying to process the next event line
    /// would result in the same issue.
    async fn handle_error(
        &self,
        _event: &Event,
        error: EventProcessorError,
    ) -> Result<(), EventProcessorError> {
        if matches!(error, EventProcessorError::MissingDependency { .. }) {
            // TODO save event in retry manager
            Ok(())
        } else if error.is_infrastructure() {
            return Err(error);
        } else {
            Ok(())
        }
    }

    /// Processes an event and delegates to [`Self::handle_error`] on failure.
    #[tracing::instrument(
        name = "event.process",
        skip_all,
        fields(
            event.resource = %event.parsed_uri.resource,
            event.uri = %event.uri,
            event.r#type = %event.event_type,
            event.user_id = %event.parsed_uri.user_id,
            event.resource_id = event.parsed_uri.resource.id().unwrap_or_default(),
            // homeserver = %self.homeserver.id, // TODO Related to X1
            otel.status_code = tracing::field::Empty,
            otel.status_message = tracing::field::Empty,
        )
    )]
    async fn handle_event(&self, event: &Event) -> Result<(), EventProcessorError> {
        let span = tracing::Span::current();
        if let Err(e) = handle(event, self.moderation().clone()).await {
            span.record("otel.status_code", "ERROR");
            span.record("otel.status_message", tracing::field::display(&e));

            self.handle_error(event, e).await?;
        } else {
            span.record("otel.status_code", "OK");
        }

        Ok(())
    }
}
