use std::sync::{Arc, Mutex};

use pubky::{EventCursor, PublicKey};
use tokio::sync::watch::Receiver;
use tracing::{debug, warn};

use super::{
    invalid_data, run_recording, validate_limit, Missing, WatchOutcome, WatcherError,
    DEFAULT_EVENTS_LIMIT,
};
use crate::events::{EventBatch, HomeserverEvent};
use crate::processor::{RunOutcome, TEventProcessor};
use crate::traits::{EventHandler, LineParseOutcome, ParseFromLine};
use crate::HomeserverEventSource;

/// Builds a [`HomeserverWatcher`]: see [`Watcher::homeserver`](super::Watcher::homeserver).
pub struct HomeserverWatcherBuilder<H> {
    homeserver: PublicKey,
    handler: H,
    events_limit: u16,
    source: Arc<dyn HomeserverEventSource>,
}

impl HomeserverWatcherBuilder<Missing> {
    pub(super) fn new<C>(client: C, homeserver: PublicKey) -> Self
    where
        C: HomeserverEventSource,
    {
        Self {
            homeserver,
            handler: Missing,
            events_limit: DEFAULT_EVENTS_LIMIT,
            source: Arc::new(client),
        }
    }
}

impl<H> HomeserverWatcherBuilder<H> {
    /// The most events a run requests ([`DEFAULT_EVENTS_LIMIT`] by default). Zero is
    /// rejected by [`Self::build`].
    pub fn events_limit(mut self, limit: u16) -> Self {
        self.events_limit = limit;
        self
    }

    /// The handler each event goes to.
    pub fn handler<NewH>(self, handler: NewH) -> HomeserverWatcherBuilder<NewH>
    where
        NewH: EventHandler<HomeserverEvent, WatcherError> + 'static,
    {
        HomeserverWatcherBuilder {
            homeserver: self.homeserver,
            handler,
            events_limit: self.events_limit,
            source: self.source,
        }
    }
}

impl<H> HomeserverWatcherBuilder<H>
where
    H: EventHandler<HomeserverEvent, WatcherError> + 'static,
{
    /// Builds the watcher, which stops reading once `shutdown_rx` turns `true`.
    pub fn build(self, shutdown_rx: Receiver<bool>) -> Result<HomeserverWatcher, WatcherError> {
        validate_limit(self.events_limit)?;
        Ok(HomeserverWatcher {
            config: Arc::new(Config {
                homeserver_id: self.homeserver.z32(),
                homeserver: self.homeserver,
                handler: Arc::new(self.handler),
                events_limit: self.events_limit,
                source: self.source,
                shutdown_rx,
            }),
        })
    }
}

struct Config {
    homeserver: PublicKey,
    homeserver_id: String,
    handler: Arc<dyn EventHandler<HomeserverEvent, WatcherError>>,
    events_limit: u16,
    source: Arc<dyn HomeserverEventSource>,
    shutdown_rx: Receiver<bool>,
}

/// Reads a homeserver's `/events/` feed, one batch per [`Self::run`].
pub struct HomeserverWatcher {
    config: Arc<Config>,
}

impl HomeserverWatcher {
    /// Reads the batch after `cursor`, hands each event to the handler in order, and returns
    /// the cursor to start the next run from.
    ///
    /// The batch's cursor is checked before any event is handled: it must be a number, and
    /// move past `cursor` when the batch has events. An idle homeserver answers no events,
    /// and the cursor stays where it was. A handler error fails the run.
    pub async fn run(&self, cursor: EventCursor) -> Result<WatchOutcome, WatcherError> {
        let run = Arc::new(Run {
            config: self.config.clone(),
            cursor,
            outcome: Mutex::new(None),
        });
        run_recording(run, |run: &Run| &run.outcome).await
    }
}

/// One run of a [`HomeserverWatcher`], as a processor.
struct Run {
    config: Arc<Config>,
    cursor: EventCursor,
    outcome: Mutex<Option<WatchOutcome>>,
}

#[async_trait::async_trait]
impl TEventProcessor<HomeserverEvent, WatcherError> for Run {
    fn instance_name(&self) -> &'static str {
        "HomeserverWatcher"
    }

    fn homeserver_id(&self) -> &str {
        &self.config.homeserver_id
    }

    async fn handle(&self, event: &HomeserverEvent) -> Result<(), WatcherError> {
        self.config.handler.handle(event).await
    }

    async fn run_internal(self: Arc<Self>) -> Result<RunOutcome, WatcherError> {
        let outcome = self.read().await?;
        *self
            .outcome
            .lock()
            .map_err(|_| "the watcher run's outcome is poisoned")? = Some(outcome);
        Ok(RunOutcome::Completed)
    }
}

impl Run {
    async fn read(&self) -> Result<WatchOutcome, WatcherError> {
        let config = &self.config;
        if *config.shutdown_rx.borrow() {
            return Ok(WatchOutcome::interrupted(self.cursor, 0));
        }

        debug!(
            cursor = self.cursor.id(),
            limit = config.events_limit,
            "Polling events"
        );
        let lines = config
            .source
            .poll_events(&config.homeserver, self.cursor.id(), config.events_limit)
            .await?;
        if lines.is_empty() {
            debug!("No new events");
            return Ok(WatchOutcome::completed(self.cursor, 0));
        }

        let batch = EventBatch::split(&lines);
        let next_cursor = next_cursor(self.cursor, &batch)?;

        let mut processed_events = 0;
        for line in &batch.event_lines {
            if *config.shutdown_rx.borrow() {
                debug!(
                    processed_events,
                    "Shutdown detected; the batch is read again next run"
                );
                return Ok(WatchOutcome::interrupted(self.cursor, processed_events));
            }

            match HomeserverEvent::parse_line(line) {
                Ok(LineParseOutcome::Parsed(event)) => {
                    self.handle_event(&event).await?;
                    processed_events += 1;
                }
                Ok(LineParseOutcome::Skipped) => {}
                Ok(LineParseOutcome::Unrecognized { reason }) => {
                    warn!(%reason, "Skipping an unrecognized event line");
                }
                Err(never) => match never {},
            }
        }

        Ok(WatchOutcome::completed(next_cursor, processed_events))
    }
}

/// The cursor closing `batch`, read after `requested`: a number, past `requested` when the
/// batch has events, and `requested` itself when it has none. A homeserver that sent events
/// without a cursor, or one that would hold the cursor, would have the next run read the
/// same batch again.
fn next_cursor(
    requested: EventCursor,
    batch: &EventBatch<'_>,
) -> Result<EventCursor, WatcherError> {
    let Some(raw) = batch.cursor else {
        return Err(invalid_data(
            "the homeserver sent events without a cursor line".into(),
        ));
    };
    let id: u64 = raw.parse().map_err(|_| {
        invalid_data(format!(
            "the homeserver sent a cursor that is no number: {raw:?}"
        ))
    })?;

    if batch.has_events() && id <= requested.id() {
        return Err(invalid_data(format!(
            "the homeserver sent events with cursor {id}, which does not move past {}",
            requested.id()
        )));
    }
    if !batch.has_events() && id != requested.id() {
        return Err(invalid_data(format!(
            "the homeserver sent no events but moved the cursor from {} to {id}",
            requested.id()
        )));
    }

    Ok(EventCursor::new(id))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use pubky::{EventCursor, Keypair, PublicKey};
    use tokio::sync::watch;

    use crate::{
        EventHandler, EventMethod, HomeserverEvent, HomeserverEventSource, HsError, HsErrorKind,
        HsOp, Watcher, WatcherError,
    };

    /// Answers every poll with `lines`, recording the cursors and limits asked for.
    struct Lines {
        lines: Vec<String>,
        polls: Mutex<Vec<(u64, u16)>>,
    }

    impl Lines {
        fn new(lines: &[&str]) -> &'static Self {
            leak(Self {
                lines: lines.iter().map(|line| line.to_string()).collect(),
                polls: Mutex::default(),
            })
        }
    }

    #[async_trait::async_trait]
    impl HomeserverEventSource for &'static Lines {
        async fn poll_events(
            &self,
            _hs: &PublicKey,
            cursor: u64,
            limit: u16,
        ) -> Result<Vec<String>, HsError> {
            self.polls.lock().unwrap().push((cursor, limit));
            Ok(self.lines.clone())
        }
    }

    /// Fails every poll, as a homeserver that is down.
    struct Down;

    #[async_trait::async_trait]
    impl HomeserverEventSource for Down {
        async fn poll_events(
            &self,
            hs: &PublicKey,
            _cursor: u64,
            _limit: u16,
        ) -> Result<Vec<String>, HsError> {
            Err(HsError::new(
                hs,
                HsOp::PollEvents,
                HsErrorKind::ServerError(503, "down".into()),
            ))
        }
    }

    /// Records the events it gets. Shuts the watcher down after `shutdown_after` of them, or
    /// fails the one numbered `fail_at`.
    #[derive(Default)]
    struct Recorder {
        events: Mutex<Vec<HomeserverEvent>>,
        shutdown_after: Option<(usize, watch::Sender<bool>)>,
        fail_at: Option<usize>,
    }

    #[async_trait::async_trait]
    impl EventHandler<HomeserverEvent, WatcherError> for &'static Recorder {
        async fn handle(&self, event: &HomeserverEvent) -> Result<(), WatcherError> {
            let mut events = self.events.lock().unwrap();
            if self.fail_at == Some(events.len()) {
                return Err("handler failed".into());
            }
            events.push(event.clone());
            if let Some((after, shutdown)) = &self.shutdown_after {
                if events.len() == *after {
                    shutdown.send(true).unwrap();
                }
            }
            Ok(())
        }
    }

    fn leak<T>(value: T) -> &'static T {
        Box::leak(Box::new(value))
    }

    fn homeserver() -> PublicKey {
        Keypair::random().public_key()
    }

    async fn run(
        source: impl HomeserverEventSource,
        handler: &'static Recorder,
        cursor: u64,
    ) -> Result<crate::WatchOutcome, WatcherError> {
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        Watcher::homeserver(source, homeserver())
            .handler(handler)
            .build(shutdown_rx)?
            .run(EventCursor::new(cursor))
            .await
    }

    /// The events are handed over in order, and the cursor moves to the batch's.
    #[tokio::test]
    async fn a_batch_is_handled_and_moves_the_cursor() {
        let handler = leak(Recorder::default());
        let source = Lines::new(&["PUT pubky://a/pub/x", "DEL pubky://a/pub/y", "cursor: 12"]);

        let outcome = run(source, handler, 10).await.unwrap();

        assert_eq!(outcome.cursor, EventCursor::new(12));
        assert_eq!(outcome.processed_events, 2);
        assert!(outcome.completed);
        let events = handler.events.lock().unwrap();
        assert_eq!(events[0].method, EventMethod::Put);
        assert_eq!(events[1].uri, "pubky://a/pub/y");
    }

    /// Unrecognized lines are skipped, and still let the cursor move.
    #[tokio::test]
    async fn unrecognized_lines_are_skipped() {
        let handler = leak(Recorder::default());
        let source = Lines::new(&["garbage", "PUT pubky://a/pub/x", "cursor: 11"]);

        let outcome = run(source, handler, 10).await.unwrap();

        assert_eq!(outcome.cursor, EventCursor::new(11));
        assert_eq!(outcome.processed_events, 1);
    }

    /// An idle homeserver answers no lines, or only the cursor it was asked from: no events,
    /// and the cursor stays.
    #[tokio::test]
    async fn an_idle_homeserver_keeps_the_cursor() {
        for lines in [vec![], vec!["cursor: 7"]] {
            let handler = leak(Recorder::default());

            let outcome = run(Lines::new(&lines), handler, 7).await.unwrap();

            assert_eq!(outcome.cursor, EventCursor::new(7), "{lines:?}");
            assert_eq!(outcome.processed_events, 0);
            assert!(outcome.completed);
        }
    }

    /// A batch whose cursor is missing, no number, does not move past the one asked from, or
    /// moves without events fails the run before any event is handled.
    #[tokio::test]
    async fn a_batch_with_a_bad_cursor_fails_before_any_event() {
        for lines in [
            vec!["PUT pubky://a/pub/x"],
            vec!["PUT pubky://a/pub/x", "cursor: nope"],
            vec!["PUT pubky://a/pub/x", "cursor: 10"],
            vec!["PUT pubky://a/pub/x", "cursor: 9"],
            vec!["cursor: 11"],
        ] {
            let handler = leak(Recorder::default());

            let result = run(Lines::new(&lines), handler, 10).await;

            assert!(result.is_err(), "{lines:?}");
            assert!(handler.events.lock().unwrap().is_empty(), "{lines:?}");
        }
    }

    /// A run stopped by shutdown is not an error. It keeps the cursor it started from, so
    /// the next run reads the batch again and no event is skipped.
    #[tokio::test]
    async fn a_run_stopped_by_shutdown_keeps_the_cursor() {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let handler = leak(Recorder {
            shutdown_after: Some((1, shutdown_tx)),
            ..Recorder::default()
        });
        let source = Lines::new(&["PUT pubky://a/pub/x", "PUT pubky://a/pub/y", "cursor: 12"]);

        let outcome = Watcher::homeserver(source, homeserver())
            .handler(handler)
            .build(shutdown_rx)
            .unwrap()
            .run(EventCursor::new(10))
            .await
            .unwrap();

        assert!(!outcome.completed);
        assert_eq!(outcome.cursor, EventCursor::new(10));
        assert_eq!(outcome.processed_events, 1);
    }

    /// A watcher shut down before the run does not poll.
    #[tokio::test]
    async fn a_run_after_shutdown_does_not_poll() {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        shutdown_tx.send(true).unwrap();
        let source = Lines::new(&["PUT pubky://a/pub/x", "cursor: 12"]);

        let outcome = Watcher::homeserver(source, homeserver())
            .handler(leak(Recorder::default()))
            .build(shutdown_rx)
            .unwrap()
            .run(EventCursor::new(10))
            .await
            .unwrap();

        assert!(!outcome.completed);
        assert_eq!(outcome.cursor, EventCursor::new(10));
        assert!(source.polls.lock().unwrap().is_empty());
    }

    /// A failed poll or a failed handler fails the run.
    #[tokio::test]
    async fn a_failed_poll_or_handler_fails_the_run() {
        let error = run(Down, leak(Recorder::default()), 10).await.unwrap_err();
        assert!(error.downcast_ref::<HsError>().is_some(), "{error}");

        let failing = leak(Recorder {
            fail_at: Some(0),
            ..Recorder::default()
        });
        let source = Lines::new(&["PUT pubky://a/pub/x", "cursor: 11"]);
        let error = run(source, failing, 10).await.unwrap_err();
        assert_eq!(error.to_string(), "handler failed");
    }

    /// The limit is passed to the homeserver, and zero is rejected at build time.
    #[tokio::test]
    async fn the_events_limit_is_requested_and_must_not_be_zero() {
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        let source = Lines::new(&[]);

        Watcher::homeserver(source, homeserver())
            .handler(leak(Recorder::default()))
            .events_limit(7)
            .build(shutdown_rx.clone())
            .unwrap()
            .run(EventCursor::new(3))
            .await
            .unwrap();
        assert_eq!(*source.polls.lock().unwrap(), [(3, 7)]);

        let zero = Watcher::homeserver(Lines::new(&[]), homeserver())
            .handler(leak(Recorder::default()))
            .events_limit(0)
            .build(shutdown_rx);
        assert!(zero.is_err());
    }
}
