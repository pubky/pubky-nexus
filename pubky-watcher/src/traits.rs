//! Traits an application implements to plug into the processor.

/// How a single homeserver event line parsed.
#[derive(Debug)]
pub enum LineParseOutcome<E> {
    /// A line the application handles.
    Parsed(E),
    /// A well-formed line the application ignores.
    Skipped,
    /// A line naming a resource the application does not recognise.
    Unrecognized { reason: String },
}

/// Parses a homeserver event line (`PUT <uri>` / `DEL <uri>`) into the application's event
/// type. A line that cannot be parsed at all is an error; the processor logs it and moves on,
/// since event lines come from untrusted homeservers.
pub trait ParseFromLine: Sized {
    type Error: std::fmt::Display;

    fn parse_line(line: &str) -> Result<LineParseOutcome<Self>, Self::Error>;
}

/// Handles one parsed event: the application's meaning of it, e.g. indexing or storing it.
///
/// The handler [`Watcher`](crate::Watcher)'s builders take. A failure fails the run, so the
/// application starts the next one from the cursor it last persisted.
#[async_trait::async_trait]
pub trait EventHandler<E, Err>: Send + Sync {
    async fn handle(&self, event: &E) -> Result<(), Err>;
}

/// Classifies a failed event for [`dispatch_retryable_error`](super::dispatch_retryable_error).
///
/// Opt-in: [`TEventProcessor`](super::TEventProcessor) does not require it. A processor that
/// wants to queue failures implements it for its error type and calls
/// `dispatch_retryable_error` from an overridden
/// [`handle_error`](super::TEventProcessor::handle_error).
pub trait RetryableError: std::fmt::Display + Send + Sync {
    /// Whether processing should stop, and the error be returned to the caller, instead of
    /// being handled as one event's failure.
    fn should_not_retry_now(&self) -> bool;

    /// Whether the event failed because something it depends on is not indexed yet.
    fn is_missing_dependency(&self) -> bool;

    /// Whether the event is worth retrying later. One that is not is dropped.
    fn should_enqueue_for_retry(&self) -> bool;
}

/// Queues a failed event for a later retry. The implementation knows where the event came
/// from, so it can correlate the retry with its source.
///
/// Used by [`dispatch_retryable_error`](super::dispatch_retryable_error) once an error is
/// classified as worth retrying. The two methods let an implementation schedule a missing
/// dependency and another transient failure differently.
#[async_trait::async_trait]
pub trait EventRetryScheduler<E, Err>: Send + Sync {
    /// Queues an event that failed because of a missing dependency.
    async fn queue_missing_dep(&self, event: &E) -> Result<(), Err>;

    /// Queues an event that failed with another transient error.
    async fn queue_transient(&self, event: &E) -> Result<(), Err>;
}
