use std::fmt::{Debug, Display};
use std::sync::Arc;

use pubky::{EventCursor, PublicKey};
use tokio::sync::watch::Receiver;
use tracing::debug;

use super::{
    run_watcher, validate_limit, ConfigError, Missing, ProtocolError, WatchOutcome, WatcherError,
    DEFAULT_EVENTS_LIMIT,
};
use crate::processor::{RunOutcome, TEventProcessor};
use crate::traits::EventHandler;
use crate::{HsStreamEvent, KeyEventSource};

/// The paths a user's stream is read under when the builder sets none: everything they
/// publish.
pub const DEFAULT_KEY_STREAM_PATH: &str = "/pub/";

/// Builds a [`KeyStreamWatcher`]: see [`Watcher::key_stream`](super::Watcher::key_stream).
pub struct KeyStreamWatcherBuilder<H> {
    homeserver: PublicKey,
    users: Vec<(PublicKey, EventCursor)>,
    handler: H,
    events_limit: u16,
    path: String,
    source: Arc<dyn KeyEventSource>,
}

impl KeyStreamWatcherBuilder<Missing> {
    pub(super) fn new<C>(
        client: C,
        homeserver: PublicKey,
        users: Vec<(PublicKey, EventCursor)>,
    ) -> Self
    where
        C: KeyEventSource,
    {
        Self {
            homeserver,
            users,
            handler: Missing,
            events_limit: DEFAULT_EVENTS_LIMIT,
            path: DEFAULT_KEY_STREAM_PATH.to_string(),
            source: Arc::new(client),
        }
    }
}

impl<H> KeyStreamWatcherBuilder<H> {
    /// The most events a run requests per user ([`DEFAULT_EVENTS_LIMIT`] by default). Zero is
    /// rejected by [`Self::build`].
    pub fn events_limit(mut self, limit: u16) -> Self {
        self.events_limit = limit;
        self
    }

    /// Reads only the events whose resource path starts with `path`
    /// ([`DEFAULT_KEY_STREAM_PATH`] by default). A path that does not start with `/` is
    /// rejected by [`Self::build`].
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    /// The handler each event goes to: an [`EventHandler`] of [`HsStreamEvent`]s, whose
    /// error a failed run gives back as [`WatcherError::Handler`].
    pub fn handler<NewH>(self, handler: NewH) -> KeyStreamWatcherBuilder<NewH> {
        KeyStreamWatcherBuilder {
            homeserver: self.homeserver,
            users: self.users,
            handler,
            events_limit: self.events_limit,
            path: self.path,
            source: self.source,
        }
    }
}

impl<H> KeyStreamWatcherBuilder<H> {
    /// Builds the watcher, which stops reading once `shutdown_rx` turns `true`. Fails on an
    /// events limit of zero, an empty or duplicated user list, or a path that does not start
    /// with `/`.
    pub fn build<Err>(
        self,
        shutdown_rx: Receiver<bool>,
    ) -> Result<KeyStreamWatcher<Err>, ConfigError>
    where
        H: EventHandler<HsStreamEvent, Err> + 'static,
        Err: Display + Debug + Send + Sync + 'static,
    {
        validate_limit(self.events_limit)?;
        validate_users(&self.users)?;
        if !self.path.starts_with('/') {
            return Err(ConfigError::InvalidPath { path: self.path });
        }

        Ok(KeyStreamWatcher {
            config: Arc::new(Config {
                homeserver_id: self.homeserver.z32(),
                homeserver: self.homeserver,
                users: self.users,
                handler: Arc::new(self.handler),
                events_limit: self.events_limit,
                path: self.path,
                source: self.source,
                shutdown_rx,
            }),
        })
    }
}

struct Config<Err> {
    homeserver: PublicKey,
    homeserver_id: String,
    users: Vec<(PublicKey, EventCursor)>,
    handler: Arc<dyn EventHandler<HsStreamEvent, Err>>,
    events_limit: u16,
    path: String,
    source: Arc<dyn KeyEventSource>,
    shutdown_rx: Receiver<bool>,
}

/// How a run of a [`KeyStreamWatcher`] ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyStreamOutcome {
    /// The cursor to start each user's next run from, in the order the users were given,
    /// safe to persist: every event of the user before it was handled.
    pub cursors: Vec<(PublicKey, EventCursor)>,
    /// The events handed to the handler.
    pub processed_events: usize,
    /// Whether the run read every user. A run stopped by shutdown returns `false`; each
    /// user's cursor is still safe to persist.
    pub completed: bool,
}

/// Reads a known set of users' `/events-stream`s on one homeserver, one user after the other,
/// one batch each per [`Self::run`]. `Err` is its handler's error.
pub struct KeyStreamWatcher<Err> {
    config: Arc<Config<Err>>,
}

impl<Err> KeyStreamWatcher<Err>
where
    Err: Display + Debug + Send + Sync + 'static,
{
    /// Reads each user's events after their cursor, hands them to the handler in order, and
    /// returns each user's cursor to start the next run from.
    ///
    /// A user's stream is checked before any of its events is handled: no more events than
    /// requested, every event the user's own, and cursors that only move forward
    /// ([`ProtocolError`] otherwise). A stream that breaks this, or a handler error, fails
    /// the run.
    pub async fn run(&self) -> Result<KeyStreamOutcome, WatcherError<Err>> {
        let run = Arc::new(Run {
            config: self.config.clone(),
        });
        let read_run = run.clone();
        run_watcher(run.instance_name(), run.homeserver_id(), async move {
            read_run.read().await
        })
        .await
    }
}

/// One run of a [`KeyStreamWatcher`], as a processor, so each event is handled in its
/// [`TEventProcessor::event_span`].
struct Run<Err> {
    config: Arc<Config<Err>>,
}

#[async_trait::async_trait]
impl<Err> TEventProcessor<HsStreamEvent, WatcherError<Err>> for Run<Err>
where
    Err: Display + Debug + Send + Sync + 'static,
{
    fn instance_name(&self) -> &'static str {
        "KeyStreamWatcher"
    }

    fn homeserver_id(&self) -> &str {
        &self.config.homeserver_id
    }

    async fn handle(&self, event: &HsStreamEvent) -> Result<(), WatcherError<Err>> {
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
    async fn read(&self) -> Result<KeyStreamOutcome, WatcherError<Err>> {
        let mut cursors = self.config.users.clone();
        let mut processed_events = 0;

        for (user, cursor) in cursors.iter_mut() {
            let read = self.read_user(user, *cursor).await?;
            *cursor = read.cursor;
            processed_events += read.processed_events;
            if !read.completed {
                return Ok(KeyStreamOutcome {
                    cursors,
                    processed_events,
                    completed: false,
                });
            }
        }

        Ok(KeyStreamOutcome {
            cursors,
            processed_events,
            completed: true,
        })
    }

    async fn read_user(
        &self,
        user: &PublicKey,
        cursor: EventCursor,
    ) -> Result<WatchOutcome, WatcherError<Err>> {
        let config = &self.config;
        if *config.shutdown_rx.borrow() {
            return Ok(WatchOutcome::interrupted(cursor, 0));
        }

        debug!(user = %user.z32(), cursor = cursor.id(), "Reading the user's events");
        let events = config
            .source
            .user_events(
                &config.homeserver,
                user,
                cursor.id(),
                config.events_limit,
                &config.path,
            )
            .await?;
        validate_events(user, cursor, config.events_limit, &events)?;

        let mut safe_cursor = cursor;
        let mut processed_events = 0;
        for event in &events {
            if *config.shutdown_rx.borrow() {
                debug!(user = %user.z32(), processed_events, "Shutdown detected");
                return Ok(WatchOutcome::interrupted(safe_cursor, processed_events));
            }

            self.handle_event(event).await?;
            safe_cursor = EventCursor::new(event.cursor);
            processed_events += 1;
        }

        Ok(WatchOutcome::completed(safe_cursor, processed_events))
    }
}

fn validate_users(users: &[(PublicKey, EventCursor)]) -> Result<(), ConfigError> {
    if users.is_empty() {
        return Err(ConfigError::NoUsers);
    }
    for (index, (user, _)) in users.iter().enumerate() {
        if users[..index].iter().any(|(earlier, _)| earlier == user) {
            return Err(ConfigError::DuplicateUser { user: user.clone() });
        }
    }
    Ok(())
}

/// Checks `user`'s stream, read after `requested` with `limit`: no more events than
/// requested, every event owned by `user`, so that one user's stream cannot inject another's
/// events, and cursors that only move forward.
fn validate_events(
    user: &PublicKey,
    requested: EventCursor,
    limit: u16,
    events: &[HsStreamEvent],
) -> Result<(), ProtocolError> {
    if events.len() > limit as usize {
        return Err(ProtocolError::TooManyEvents {
            user: user.clone(),
            got: events.len(),
            limit,
        });
    }

    let mut floor = requested.id();
    for event in events {
        if event.owner() != user {
            return Err(ProtocolError::ForeignEvent {
                user: user.clone(),
                owner: event.owner().clone(),
            });
        }
        if event.cursor <= floor {
            return Err(ProtocolError::CursorNotAdvanced {
                user: Some(user.clone()),
                requested: floor,
                got: event.cursor,
            });
        }
        floor = event.cursor;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use pubky::{EventCursor, Keypair, PubkyResource, PublicKey};
    use tokio::sync::watch;

    use crate::{
        ConfigError, EventHandler, EventMethod, HsError, HsErrorKind, HsOp, HsStreamEvent,
        KeyEventSource, ProtocolError, Watcher, WatcherError,
    };

    fn key() -> PublicKey {
        Keypair::random().public_key()
    }

    /// An event of `owner`'s at `cursor`.
    fn event(owner: &PublicKey, cursor: u64) -> HsStreamEvent {
        let resource = PubkyResource::new(owner.clone(), "/pub/app/item").expect("valid path");
        HsStreamEvent::new(EventMethod::Put, &resource, cursor)
    }

    /// Answers each user with their scripted events, recording the requests.
    #[derive(Default)]
    struct Streams {
        events: HashMap<PublicKey, Vec<HsStreamEvent>>,
        requests: Mutex<Vec<(PublicKey, u64, u16, String)>>,
    }

    impl Streams {
        fn new(events: Vec<(&PublicKey, Vec<HsStreamEvent>)>) -> &'static Self {
            Box::leak(Box::new(Self {
                events: events
                    .into_iter()
                    .map(|(user, events)| (user.clone(), events))
                    .collect(),
                requests: Mutex::default(),
            }))
        }
    }

    #[async_trait::async_trait]
    impl KeyEventSource for &'static Streams {
        async fn user_events(
            &self,
            _hs: &PublicKey,
            user: &PublicKey,
            cursor: u64,
            limit: u16,
            path: &str,
        ) -> Result<Vec<HsStreamEvent>, HsError> {
            self.requests
                .lock()
                .unwrap()
                .push((user.clone(), cursor, limit, path.to_string()));
            Ok(self.events.get(user).cloned().unwrap_or_default())
        }
    }

    /// Records the cursors of the events it gets, and shuts the watcher down after
    /// `shutdown_after` of them.
    #[derive(Default)]
    struct Recorder {
        cursors: Mutex<Vec<u64>>,
        shutdown_after: Option<(usize, watch::Sender<bool>)>,
    }

    #[async_trait::async_trait]
    impl EventHandler<HsStreamEvent, String> for &'static Recorder {
        async fn handle(&self, event: &HsStreamEvent) -> Result<(), String> {
            let mut cursors = self.cursors.lock().unwrap();
            cursors.push(event.cursor);
            if let Some((after, shutdown)) = &self.shutdown_after {
                if cursors.len() == *after {
                    shutdown.send(true).unwrap();
                }
            }
            Ok(())
        }
    }

    fn recorder() -> &'static Recorder {
        Box::leak(Box::default())
    }

    /// Each user's events are handled, and each user moves to their own last cursor.
    #[tokio::test]
    async fn each_user_moves_to_their_own_cursor() {
        let (alice, bob, carol) = (key(), key(), key());
        let source = Streams::new(vec![
            (&alice, vec![event(&alice, 11), event(&alice, 12)]),
            (&bob, vec![event(&bob, 31)]),
        ]);
        let handler = recorder();
        let users = vec![
            (alice.clone(), EventCursor::new(10)),
            (bob.clone(), EventCursor::new(30)),
            (carol.clone(), EventCursor::new(50)),
        ];
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);

        let outcome = Watcher::key_stream(source, key(), users)
            .handler(handler)
            .build(shutdown_rx)
            .unwrap()
            .run()
            .await
            .unwrap();

        assert!(outcome.completed);
        assert_eq!(outcome.processed_events, 3);
        assert_eq!(
            outcome.cursors,
            vec![
                (alice, EventCursor::new(12)),
                (bob, EventCursor::new(31)),
                (carol, EventCursor::new(50)),
            ]
        );
        assert_eq!(*handler.cursors.lock().unwrap(), [11, 12, 31]);
    }

    /// Each request asks for the user's cursor, the limit and the path: `/pub/` unless set.
    #[tokio::test]
    async fn requests_carry_the_cursor_limit_and_path() {
        let alice = key();
        for (path, expected) in [(None, "/pub/"), (Some("/pub/my.app/"), "/pub/my.app/")] {
            let source = Streams::new(vec![]);
            let (_shutdown_tx, shutdown_rx) = watch::channel(false);
            let mut builder =
                Watcher::key_stream(source, key(), vec![(alice.clone(), EventCursor::new(4))])
                    .handler(recorder())
                    .events_limit(9);
            if let Some(path) = path {
                builder = builder.path(path);
            }

            builder.build(shutdown_rx).unwrap().run().await.unwrap();

            assert_eq!(
                *source.requests.lock().unwrap(),
                [(alice.clone(), 4, 9, expected.to_string())]
            );
        }
    }

    /// A stream with more events than requested, an event of another user, or a cursor that
    /// does not move forward fails the run before any of its events is handled.
    #[tokio::test]
    async fn a_stream_that_breaks_the_protocol_fails_before_any_event() {
        let (alice, mallory) = (key(), key());
        let not_advanced = |requested, got| ProtocolError::CursorNotAdvanced {
            user: Some(alice.clone()),
            requested,
            got,
        };
        for (events, expected) in [
            (
                vec![event(&alice, 11), event(&alice, 12), event(&alice, 13)],
                ProtocolError::TooManyEvents {
                    user: alice.clone(),
                    got: 3,
                    limit: 2,
                },
            ),
            (
                vec![event(&alice, 11), event(&mallory, 12)],
                ProtocolError::ForeignEvent {
                    user: alice.clone(),
                    owner: mallory.clone(),
                },
            ),
            (vec![event(&alice, 10)], not_advanced(10, 10)),
            (
                vec![event(&alice, 12), event(&alice, 11)],
                not_advanced(12, 11),
            ),
        ] {
            let source = Streams::new(vec![(&alice, events)]);
            let handler = recorder();
            let (_shutdown_tx, shutdown_rx) = watch::channel(false);

            let result =
                Watcher::key_stream(source, key(), vec![(alice.clone(), EventCursor::new(10))])
                    .handler(handler)
                    .events_limit(2)
                    .build(shutdown_rx)
                    .unwrap()
                    .run()
                    .await;

            assert!(
                matches!(&result, Err(WatcherError::Protocol(got)) if *got == expected),
                "{result:?}"
            );
            assert!(handler.cursors.lock().unwrap().is_empty());
        }
    }

    /// A run stopped by shutdown is not an error: the user in progress keeps the cursor of
    /// the last event handled, and the users after them keep theirs.
    #[tokio::test]
    async fn a_run_stopped_by_shutdown_keeps_safe_cursors() {
        let (alice, bob) = (key(), key());
        let source = Streams::new(vec![
            (&alice, vec![event(&alice, 11), event(&alice, 12)]),
            (&bob, vec![event(&bob, 31)]),
        ]);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let handler: &'static Recorder = Box::leak(Box::new(Recorder {
            shutdown_after: Some((1, shutdown_tx)),
            ..Recorder::default()
        }));
        let users = vec![
            (alice.clone(), EventCursor::new(10)),
            (bob.clone(), EventCursor::new(30)),
        ];

        let outcome = Watcher::key_stream(source, key(), users)
            .handler(handler)
            .build(shutdown_rx)
            .unwrap()
            .run()
            .await
            .unwrap();

        assert!(!outcome.completed);
        assert_eq!(outcome.processed_events, 1);
        assert_eq!(
            outcome.cursors,
            vec![(alice, EventCursor::new(11)), (bob, EventCursor::new(30))]
        );
    }

    /// Fails every stream request, as a homeserver that is down.
    struct Down;

    #[async_trait::async_trait]
    impl KeyEventSource for Down {
        async fn user_events(
            &self,
            hs: &PublicKey,
            _user: &PublicKey,
            _cursor: u64,
            _limit: u16,
            _path: &str,
        ) -> Result<Vec<HsStreamEvent>, HsError> {
            Err(HsError::new(
                hs,
                HsOp::UserEvents,
                HsErrorKind::ServerError(503, "down".into()),
            ))
        }
    }

    /// The handler's own error: the cursor of the event it failed on.
    #[derive(Debug, PartialEq, Eq)]
    struct FailedAt(u64);

    impl std::fmt::Display for FailedAt {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "handler failed at cursor {}", self.0)
        }
    }

    /// Records the cursors of the events it gets, and fails on the event at `fail_at`.
    struct FailingAt {
        fail_at: u64,
        cursors: Mutex<Vec<u64>>,
    }

    #[async_trait::async_trait]
    impl EventHandler<HsStreamEvent, FailedAt> for &'static FailingAt {
        async fn handle(&self, event: &HsStreamEvent) -> Result<(), FailedAt> {
            if event.cursor == self.fail_at {
                return Err(FailedAt(event.cursor));
            }
            self.cursors.lock().unwrap().push(event.cursor);
            Ok(())
        }
    }

    /// A handler error fails the run with the handler's own error, typed; the events before
    /// it were handled.
    #[tokio::test]
    async fn a_handler_failure_fails_the_run_with_its_own_error() {
        let alice = key();
        let source = Streams::new(vec![(
            &alice,
            vec![event(&alice, 11), event(&alice, 12), event(&alice, 13)],
        )]);
        let handler: &'static FailingAt = Box::leak(Box::new(FailingAt {
            fail_at: 12,
            cursors: Mutex::default(),
        }));
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);

        let error = Watcher::key_stream(source, key(), vec![(alice, EventCursor::new(10))])
            .handler(handler)
            .build(shutdown_rx)
            .unwrap()
            .run()
            .await
            .unwrap_err();

        assert!(
            matches!(error, WatcherError::Handler(FailedAt(12))),
            "{error:?}"
        );
        assert_eq!(*handler.cursors.lock().unwrap(), [11]);
    }

    /// A failed stream request fails the run as the homeserver's error, before any event.
    #[tokio::test]
    async fn a_failed_stream_request_fails_the_run() {
        let handler = recorder();
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);

        let error = Watcher::key_stream(Down, key(), vec![(key(), EventCursor::new(10))])
            .handler(handler)
            .build(shutdown_rx)
            .unwrap()
            .run()
            .await
            .unwrap_err();

        assert!(
            matches!(&error, WatcherError::Homeserver(e) if e.op == HsOp::UserEvents),
            "{error:?}"
        );
        assert!(handler.cursors.lock().unwrap().is_empty());
    }

    /// An events limit of zero, no users, a user listed twice, or a path that does not start
    /// with `/` is rejected at build time.
    #[test]
    fn a_misconfigured_watcher_is_rejected_at_build_time() {
        let alice = key();
        let one = || vec![(alice.clone(), EventCursor::new(0))];
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        let build = |users, limit, path: &str| {
            Watcher::key_stream(Streams::new(vec![]), key(), users)
                .handler(recorder())
                .events_limit(limit)
                .path(path)
                .build(shutdown_rx.clone())
        };

        assert!(build(one(), 1, "/pub/").is_ok());
        assert_eq!(
            build(one(), 0, "/pub/").err(),
            Some(ConfigError::ZeroEventsLimit)
        );
        assert_eq!(build(vec![], 1, "/pub/").err(), Some(ConfigError::NoUsers));
        assert_eq!(
            build([one(), one()].concat(), 1, "/pub/").err(),
            Some(ConfigError::DuplicateUser {
                user: alice.clone()
            })
        );
        assert_eq!(
            build(one(), 1, "pub/").err(),
            Some(ConfigError::InvalidPath {
                path: "pub/".into()
            })
        );
    }
}
