//! One run over one homeserver: [`TEventProcessor`] reads a homeserver's events and hands
//! each to the application, on its own task ([`run_on_own_task`]), with opt-in retry through
//! [`dispatch_retryable_error`].
//!
//! Implement it when the [`Watcher`](crate::Watcher) builders are not enough: your own
//! cursor store, your own event type, retries, or many homeservers under a
//! [`TEventProcessorRunner`](crate::TEventProcessorRunner).

use std::fmt::{Debug, Display};
use std::{future::Future, sync::Arc, time::Duration};

use tracing::{debug, error, trace, warn, Instrument};

use super::traits::{EventRetryScheduler, LineParseOutcome, ParseFromLine, RetryableError};

/// How long a processor run may take before it is logged as slow, unless the processor sets
/// its own ([`TEventProcessor::slow_run_threshold`]).
// TODO(#1014): read it, and the run budget, from the config file
pub const SLOW_RUN_THRESHOLD: Duration = Duration::from_secs(3_600);

/// How a run of an event processor ended when it did not fail.
/// Returned by [`TEventProcessor::run_internal`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    /// The run ended without failing or spending its budget: it did the work it found, or
    /// stopped early for shutdown.
    /// [`TEventProcessor::run`] then returns `Ok`.
    Completed,
    /// The run stopped at its budget with work left, keeping the cursors it reached. Not an
    /// event failure: nothing is retried.
    /// [`TEventProcessor::run`] then returns [`RunError::BudgetSpent`].
    BudgetSpent,
}

/// Why an event processor run did not complete.
/// Returned by [`TEventProcessor::run`] and [`run_on_own_task`].
#[derive(Debug)]
pub enum RunError<Err> {
    /// [`TEventProcessor::run_internal`] failed with the processor's own error.
    Internal(Err),
    /// The run's task panicked: see [`run_on_own_task`].
    Panicked,
    /// The run stopped at its budget, see [`RunOutcome::BudgetSpent`]
    BudgetSpent,
}

impl<Err> RunError<Err> {
    /// Whether the run panicked ([`RunError::Panicked`]).
    pub fn is_panic(&self) -> bool {
        matches!(self, RunError::Panicked)
    }
}

impl<Err: Debug + Display> std::error::Error for RunError<Err> {}

impl<Err: Display> Display for RunError<Err> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::Internal(err) => write!(f, "Internal error: {err}"),
            RunError::Panicked => write!(f, "Execution panicked"),
            RunError::BudgetSpent => write!(f, "Run budget spent"),
        }
    }
}

/// Classifies `error` with [`RetryableError`] and queues the event when it is worth retrying.
///
/// - An error that should not be retried now is returned, so the caller stops its batch.
/// - One not worth retrying is dropped.
/// - Otherwise the event is queued on `scheduler`, as a missing dependency or as a transient
///   failure. Without a scheduler it is dropped.
///
/// Opt-in: call it from an overridden [`TEventProcessor::handle_error`]. The default
/// `handle_error` fails fast without requiring [`RetryableError`].
pub async fn dispatch_retryable_error<E, Err>(
    event: &E,
    error: Err,
    scheduler: Option<&dyn EventRetryScheduler<E, Err>>,
) -> Result<(), Err>
where
    E: Sync,
    Err: RetryableError,
{
    if error.should_not_retry_now() {
        warn!(error = %error, "Got should-not-retry-now error, stopping batch");
        return Err(error);
    }

    if !error.should_enqueue_for_retry() {
        debug!(error = %error, "Error not worth retrying, skipping event");
        return Ok(());
    }

    let Some(scheduler) = scheduler else {
        return Ok(());
    };

    if error.is_missing_dependency() {
        scheduler.queue_missing_dep(event).await
    } else {
        warn!(error = %error, "Transient error, queuing event for retry");
        scheduler.queue_transient(event).await
    }
}

/// An event processor: one run reads a homeserver's events and hands each to the
/// application, and can be stopped gracefully through a shutdown signal the implementation
/// watches.
///
/// Generic over the application's event type `E` and error type `Err`.
/// [`TEventProcessorRunner`](crate::TEventProcessorRunner) runs one per homeserver.
#[async_trait::async_trait]
pub trait TEventProcessor<E, Err>: Send + Sync + 'static
where
    E: Send + Sync + 'static,
    Err: Display + Debug + Send + Sync + 'static,
{
    /// A short service label (e.g. `HsEventProcessor`), named in the span and logs of
    /// [`Self::run`].
    fn instance_name(&self) -> &'static str;

    /// The ID of the homeserver this processor reads events from, named in the span and logs
    /// of [`Self::run`].
    fn homeserver_id(&self) -> &str;

    /// Runs [`Self::run_internal`] on its own task, and waits for it to end: see
    /// [`run_on_own_task`].
    async fn run(self: Arc<Self>) -> Result<(), RunError<Err>> {
        let slow_run_threshold = self.slow_run_threshold().unwrap_or(SLOW_RUN_THRESHOLD);
        let instance_name = self.instance_name();
        let homeserver_id = self.homeserver_id().to_string();
        run_on_own_task(
            instance_name,
            Some(&homeserver_id),
            slow_run_threshold,
            self.run_internal(),
        )
        .await
    }

    /// Runs the event processor once; [`Self::run`] calls it on its own task.
    ///
    /// Returns how the run ended ([`RunOutcome`]), or the error it failed with.
    async fn run_internal(self: Arc<Self>) -> Result<RunOutcome, Err>;

    /// How long a run of this event processor may take before it is logged as slow.
    ///
    /// If not set, the [`SLOW_RUN_THRESHOLD`] is applied.
    fn slow_run_threshold(&self) -> Option<Duration> {
        None
    }

    /// Hands `event` to the application's handler, from [`Self::handle_event`] once
    /// [`Self::should_process_event`] accepted it.
    async fn handle(&self, event: &E) -> Result<(), Err>;

    /// Parses a single event line and dispatches to [`Self::handle_event`]. A line that does
    /// not parse, or names a resource the application does not recognise, is logged and
    /// skipped: event lines come from untrusted homeservers.
    async fn process_event_line(&self, line: &str) -> Result<(), Err>
    where
        E: ParseFromLine + Debug,
    {
        let outcome = match E::parse_line(line) {
            Ok(outcome) => outcome,
            // Invalid event lines come from untrusted homeservers; treat as bad peer data, not
            // application errors.
            Err(e) => {
                warn!(error = %e, "Invalid event line");
                return Ok(());
            }
        };
        match outcome {
            LineParseOutcome::Skipped => {}
            LineParseOutcome::Unrecognized { reason } => {
                warn!(%reason, "Unrecognized event URI");
            }
            LineParseOutcome::Parsed(event) => {
                trace!(?event, "Processing event");
                self.handle_event(&event).await?;
            }
        }

        Ok(())
    }

    /// Handles an error of processing `event`.
    ///
    /// Called in the event processing loop. Returns:
    /// - `Ok(())` to continue processing the batch
    /// - `Err(e)` to stop processing and return the error
    ///
    /// The default fails fast. A processor that queues failures overrides it and calls
    /// [`dispatch_retryable_error`].
    async fn handle_error(&self, _event: &E, error: Err) -> Result<(), Err> {
        Err(error)
    }

    /// Decides whether `event` should be processed by this processor.
    ///
    /// Called at the start of [`Self::handle_event`], before the event is handed to
    /// [`Self::handle`]. The default always processes the event.
    async fn should_process_event(&self, _event: &E) -> Result<bool, Err> {
        Ok(true)
    }

    /// The span [`Self::handle_event`] runs in. It must declare the empty fields
    /// `otel.status_code` and `otel.status_message`, which record the event's outcome.
    fn event_span(&self, _event: &E) -> tracing::Span {
        tracing::info_span!(
            "event.process",
            otel.status_code = tracing::field::Empty,
            otel.status_message = tracing::field::Empty,
        )
    }

    /// Processes an event and delegates to [`Self::handle_error`] on failure.
    async fn handle_event(&self, event: &E) -> Result<(), Err> {
        let span = self.event_span(event);
        async {
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

            if let Err(e) = self.handle(event).await {
                span.record("otel.status_code", "ERROR");
                span.record("otel.status_message", tracing::field::display(&e));

                self.handle_error(event, e).await?;
            } else {
                span.record("otel.status_code", "OK");
            }

            Ok(())
        }
        .instrument(span)
        .await
    }
}

/// Runs `run_internal`, a run of the processor `instance_name`, on its own task, and waits for
/// it to end.
///
/// A run that takes longer than `slow_run_threshold` is logged as slow, again after each
/// further threshold, and still waited for: it is not stopped, so the next run cannot start
/// next to it. A run that never returns therefore holds up its loop for good.
///
/// `homeserver` is the homeserver the run is bound to, named in its span. A run that is not
/// bound to one homeserver names none.
/// It fails with [`RunError::Panicked`] or [`RunError::Internal`], or with
/// [`RunError::BudgetSpent`] when the run returns [`RunOutcome::BudgetSpent`].
pub async fn run_on_own_task<Err>(
    instance_name: &'static str,
    homeserver: Option<&str>,
    slow_run_threshold: Duration,
    run_internal: impl Future<Output = Result<RunOutcome, Err>> + Send + 'static,
) -> Result<(), RunError<Err>>
where
    Err: Debug + Send + 'static,
{
    match wait_on_own_task(instance_name, homeserver, slow_run_threshold, run_internal).await {
        Ok(Ok(RunOutcome::Completed)) => Ok(()),
        Ok(Ok(RunOutcome::BudgetSpent)) => Err(RunError::BudgetSpent),
        Ok(Err(e)) => Err(RunError::Internal(e)),
        Err(Panicked) => Err(RunError::Panicked),
    }
}

/// The task of a run ended without returning: it panicked.
pub(crate) struct Panicked;

/// Runs `run` on its own task as [`run_on_own_task`] does, and returns what it returned, or
/// [`Panicked`]. A run that fails is logged.
pub(crate) async fn wait_on_own_task<T, Err>(
    instance_name: &'static str,
    homeserver: Option<&str>,
    slow_run_threshold: Duration,
    run: impl Future<Output = Result<T, Err>> + Send + 'static,
) -> Result<Result<T, Err>, Panicked>
where
    T: Send + 'static,
    Err: Debug + Send + 'static,
{
    let homeserver = homeserver.map(tracing::field::display);
    let span = tracing::info_span!(
        "event_processor.run",
        service = %instance_name,
        homeserver,
    );
    let started = tokio::time::Instant::now();
    let mut handle = tokio::spawn(run.instrument(span));

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
    let result = join_result
        .inspect_err(|je| {
            error!(
                service = %instance_name,
                homeserver,
                error = ?je,
                "Event processor JoinError"
            )
        })
        .map_err(|_| Panicked)?;

    if let Err(e) = &result {
        error!(
            service = %instance_name,
            homeserver,
            error = ?e,
            "Event processor failed"
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::time::Duration;

    use super::{run_on_own_task, RunOutcome};

    /// The warning is the only sign of a run that never returns, so it repeats after every
    /// threshold, with how long the run has been going, and the run is still waited for.
    #[tokio::test(start_paused = true)]
    #[tracing_test::traced_test]
    async fn a_slow_run_is_logged_at_every_threshold_and_still_waited_for() {
        let result = run_on_own_task::<Infallible>(
            "SlowProcessor",
            Some("homeserver"),
            Duration::from_secs(60),
            async {
                tokio::time::sleep(Duration::from_secs(250)).await;
                Ok(RunOutcome::Completed)
            },
        )
        .await;

        assert!(result.is_ok(), "the run is waited for, not stopped");
        logs_assert(|lines| {
            let warned: Vec<_> = lines
                .iter()
                .filter(|line| line.contains(" WARN ") && line.contains("run is slow"))
                .collect();
            for (line, secs) in warned.iter().zip([60, 120, 180, 240]) {
                if !line.contains(&format!("running_for_secs={secs}")) {
                    return Err(format!("expected running_for_secs={secs} in {line}"));
                }
            }
            match warned.len() {
                4 => Ok(()),
                n => Err(format!("expected 4 slow-run warnings, got {n}")),
            }
        });
    }
}
