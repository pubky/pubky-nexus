use std::fmt::{Debug, Display};
use std::sync::Arc;

use pubky::{EventCursor, PublicKey};
use tokio::sync::watch::Receiver;
use tracing::{debug, warn};

use super::{
    run_watcher, validate_limit, ConfigError, Missing, ProtocolError, WatchOutcome, WatcherError,
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

    /// The handler each event goes to: an [`EventHandler`] of [`HomeserverEvent`]s, whose
    /// error a failed run gives back as [`WatcherError::Handler`].
    pub fn handler<NewH>(self, handler: NewH) -> HomeserverWatcherBuilder<NewH> {
        HomeserverWatcherBuilder {
            homeserver: self.homeserver,
            handler,
            events_limit: self.events_limit,
            source: self.source,
        }
    }

    /// Builds the watcher, which stops reading once `shutdown_rx` turns `true`. Fails on an
    /// events limit of zero.
    /// The error is a [`ConfigError`].
    pub fn build<Err>(
        self,
        shutdown_rx: Receiver<bool>,
    ) -> Result<HomeserverWatcher<Err>, ConfigError>
    where
        H: EventHandler<HomeserverEvent, Err> + 'static,
        Err: Display + Debug + Send + Sync + 'static,
    {
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

struct Config<Err> {
    homeserver: PublicKey,
    homeserver_id: String,
    handler: Arc<dyn EventHandler<HomeserverEvent, Err>>,
    events_limit: u16,
    source: Arc<dyn HomeserverEventSource>,
    shutdown_rx: Receiver<bool>,
}

/// Reads a homeserver's `/events/` feed, one batch per [`Self::run`]. `Err` is its handler's
/// error.
pub struct HomeserverWatcher<Err> {
    config: Arc<Config<Err>>,
}

impl<Err> HomeserverWatcher<Err>
where
    Err: Display + Debug + Send + Sync + 'static,
{
    /// Reads the batch after `cursor`, hands each event to the handler in order, and returns
    /// the cursor to start the next run from.
    ///
    /// The batch's cursor is checked before any event is handled: it must be a number, and
    /// move past `cursor` when the batch has events ([`ProtocolError`] otherwise). An idle
    /// homeserver answers no events, and the cursor stays where it was. A handler error fails
    /// the run.
    pub async fn run(&self, cursor: EventCursor) -> Result<WatchOutcome, WatcherError<Err>> {
        let run = Arc::new(Run {
            config: self.config.clone(),
            cursor,
        });
        let read_run = run.clone();
        run_watcher(run.instance_name(), run.homeserver_id(), async move {
            read_run.read().await
        })
        .await
    }
}

/// One run of a [`HomeserverWatcher`], as a processor, so each event is handled in its
/// [`TEventProcessor::event_span`].
struct Run<Err> {
    config: Arc<Config<Err>>,
    cursor: EventCursor,
}

#[async_trait::async_trait]
impl<Err> TEventProcessor<HomeserverEvent, WatcherError<Err>> for Run<Err>
where
    Err: Display + Debug + Send + Sync + 'static,
{
    fn instance_name(&self) -> &'static str {
        "HomeserverWatcher"
    }

    fn homeserver_id(&self) -> &str {
        &self.config.homeserver_id
    }

    async fn handle(&self, event: &HomeserverEvent) -> Result<(), WatcherError<Err>> {
        self.config
            .handler
            .handle(event)
            .await
            .map_err(WatcherError::Handler)
    }

    async fn run_internal(self: Arc<Self>) -> Result<RunOutcome, WatcherError<Err>> {
        self.read().await?;
        Ok(RunOutcome::Completed)
    }
}

impl<Err> Run<Err>
where
    Err: Display + Debug + Send + Sync + 'static,
{
    async fn read(&self) -> Result<WatchOutcome, WatcherError<Err>> {
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
) -> Result<EventCursor, ProtocolError> {
    let Some(raw) = batch.cursor else {
        return Err(ProtocolError::MissingCursor);
    };
    let got: u64 = raw.parse().map_err(|_| ProtocolError::MalformedCursor {
        raw: raw.to_string(),
    })?;

    if batch.has_events() && got <= requested.id() {
        return Err(ProtocolError::CursorNotAdvanced {
            user: None,
            requested: requested.id(),
            got,
        });
    }
    if !batch.has_events() && got != requested.id() {
        return Err(ProtocolError::CursorMovedWithoutEvents {
            requested: requested.id(),
            got,
        });
    }

    Ok(EventCursor::new(got))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use pubky::{EventCursor, Keypair, PublicKey};
    use tokio::sync::watch;

    use crate::{
        ConfigError, EventHandler, EventMethod, HomeserverEvent, HomeserverEventSource, HsError,
        HsErrorKind, HsOp, ProtocolError, Watcher, WatcherError,
    };

    /// The handler's own error: the index of the event it failed on.
    #[derive(Debug, PartialEq, Eq)]
    struct HandlerFailed(usize);

    impl std::fmt::Display for HandlerFailed {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "handler failed on event {}", self.0)
        }
    }

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
    impl EventHandler<HomeserverEvent, HandlerFailed> for &'static Recorder {
        async fn handle(&self, event: &HomeserverEvent) -> Result<(), HandlerFailed> {
            let mut events = self.events.lock().unwrap();
            if self.fail_at == Some(events.len()) {
                return Err(HandlerFailed(events.len()));
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
    ) -> Result<crate::WatchOutcome, WatcherError<HandlerFailed>> {
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        Watcher::homeserver(source, homeserver())
            .handler(handler)
            .build(shutdown_rx)
            .expect("a valid configuration")
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
        let not_advanced = |got| ProtocolError::CursorNotAdvanced {
            user: None,
            requested: 10,
            got,
        };
        for (lines, expected) in [
            (vec!["PUT pubky://a/pub/x"], ProtocolError::MissingCursor),
            (
                vec!["PUT pubky://a/pub/x", "cursor: nope"],
                ProtocolError::MalformedCursor { raw: "nope".into() },
            ),
            (vec!["PUT pubky://a/pub/x", "cursor: 10"], not_advanced(10)),
            (vec!["PUT pubky://a/pub/x", "cursor: 9"], not_advanced(9)),
            (
                vec!["cursor: 11"],
                ProtocolError::CursorMovedWithoutEvents {
                    requested: 10,
                    got: 11,
                },
            ),
        ] {
            let handler = leak(Recorder::default());

            let error = run(Lines::new(&lines), handler, 10).await.unwrap_err();

            assert!(
                matches!(&error, WatcherError::Protocol(got) if *got == expected),
                "{lines:?}: {error:?}"
            );
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
        assert!(
            matches!(&error, WatcherError::Homeserver(e) if e.op == HsOp::PollEvents),
            "{error:?}"
        );

        let failing = leak(Recorder {
            fail_at: Some(1),
            ..Recorder::default()
        });
        let source = Lines::new(&["PUT pubky://a/pub/x", "PUT pubky://a/pub/y", "cursor: 11"]);
        let error = run(source, failing, 10).await.unwrap_err();
        assert!(
            matches!(error, WatcherError::Handler(HandlerFailed(1))),
            "{error:?}"
        );
        assert_eq!(error.to_string(), "handler failed on event 1");
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
            .build::<HandlerFailed>(shutdown_rx);
        assert_eq!(zero.err(), Some(ConfigError::ZeroEventsLimit));
    }
}
