mod homeserver;
mod key_based;

pub use homeserver::HsEventProcessor;
pub use key_based::KeyBasedEventProcessor;

use std::{fmt::Display, path::PathBuf, sync::Arc, time::Duration};

use nexus_common::models::event::{Event, EventProcessorError};
use opentelemetry::trace::{FutureExt, Span, TraceContextExt, Tracer};
use opentelemetry::{global, Context, KeyValue};
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
    fn tracer_name(&self) -> &str;
    fn moderation(&self) -> &Arc<Moderation>;

    async fn run(self: Arc<Self>) -> Result<(), RunError> {
        let tracer_name = self.tracer_name().to_string();
        let timeout = self
            .custom_timeout()
            .unwrap_or(Duration::from_secs(PROCESSING_TIMEOUT_SECS));

        let handle = tokio::spawn(self.run_internal());

        let join_result = tokio::time::timeout(timeout, handle)
            .await
            .inspect_err(|_| error!("Event processor timed out for {tracer_name}"))
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
                error!("JoinError while running event processor for {tracer_name}: {je:?}")
            })
            .map_err(|_| RunError::Panicked)?;

        run_internal_result
            .inspect_err(|e| error!("Event processor failed for {tracer_name}: {e:?}"))
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
            let tracer = global::tracer(self.tracer_name().to_string());
            let mut span = tracer.start(event.parsed_uri.resource.to_string());
            span.set_attribute(KeyValue::new("event.uri", event.uri.clone()));
            span.set_attribute(KeyValue::new("event.type", event.event_type.to_string()));
            span.set_attribute(KeyValue::new(
                "event.user_id",
                event.parsed_uri.user_id.to_string(),
            ));
            span.set_attribute(KeyValue::new(
                "event.resource_id",
                event.parsed_uri.resource.id().unwrap_or("".to_string()),
            ));
            let cx = Context::new().with_span(span);
            debug!("Processing event: {:?}", event);
            self.handle_event(&event).with_context(cx).await?;
        }

        Ok(())
    }

    /// Handles an error from event processing (e.g. logging, scheduling retries).
    ///
    /// Default is a no-op. Override to implement retry strategies.
    async fn handle_error(&self, _event: &Event, _error: EventProcessorError) {}

    /// Processes an event and delegates to [`Self::handle_error`] on failure.
    async fn handle_event(&self, event: &Event) -> Result<(), EventProcessorError> {
        if let Err(e) = handle(event, self.moderation().clone()).await {
            self.handle_error(event, e).await;
        }
        Ok(())
    }
}
