mod homeserver;
mod key_based;

pub use homeserver::HsEventProcessor;
pub use key_based::KeyBasedEventProcessor;

pub use pubky_watcher::{RunOutcome, TEventProcessor};

use crate::errors::EventProcessorError;
use crate::events::retry::RetryScheduler;
use crate::events::Event;
use pubky_social_specs::PubkyId;
use pubky_watcher::{dispatch_retryable_error, EventRetryScheduler};

/// How a run of a Nexus event processor did not complete.
pub type RunError = pubky_watcher::RunError<EventProcessorError>;

/// The event processors the Nexus runners build and run.
pub type NexusEventProcessor = dyn TEventProcessor<Event, EventProcessorError>;

/// Handles a failed event the way every Nexus processor does: an error that should not be
/// retried now stops the batch, one not worth retrying is dropped, and the rest are queued on
/// `scheduler` with `homeserver_id` as their origin. Without a scheduler, as in the test
/// mocks, a retryable failure is dropped.
///
/// No flip is dropped here: its step runs before the fetch and every failure of it is queued,
/// so a `social/v1` profile PUT that is 404 or invalid is a `v1` user's own edit.
pub(crate) async fn handle_error(
    event: &Event,
    error: EventProcessorError,
    scheduler: Option<&RetryScheduler>,
    homeserver_id: &PubkyId,
) -> Result<(), EventProcessorError> {
    let scheduler = scheduler.map(|scheduler| QueueFrom {
        scheduler,
        origin: homeserver_id,
    });
    dispatch_retryable_error(
        event,
        error,
        scheduler
            .as_ref()
            .map(|s| s as &dyn EventRetryScheduler<Event, EventProcessorError>),
    )
    .await
}

/// The span a Nexus processor handles an event in, named after the event's resource.
pub(crate) fn event_span(event: &Event) -> tracing::Span {
    tracing::info_span!(
        "event.process",
        event.resource = %event.target.resource_name(),
        event.uri = %event.uri,
        event.r#type = %event.event_type,
        event.user_id = %event.target.user_id(),
        event.resource_id = event.target.resource_id(),
        otel.status_code = tracing::field::Empty,
        otel.status_message = tracing::field::Empty,
    )
}

/// Queues a processor's failed events on the retry queue, with the homeserver they were read
/// from as their origin.
struct QueueFrom<'a> {
    scheduler: &'a RetryScheduler,
    origin: &'a PubkyId,
}

#[async_trait::async_trait]
impl EventRetryScheduler<Event, EventProcessorError> for QueueFrom<'_> {
    async fn queue_missing_dep(&self, event: &Event) -> Result<(), EventProcessorError> {
        self.scheduler.queue_missing_dep(event, self.origin).await
    }

    async fn queue_transient(&self, event: &Event) -> Result<(), EventProcessorError> {
        self.scheduler.queue_transient(event, self.origin).await
    }
}
