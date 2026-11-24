use std::sync::{Arc, Mutex};

use pubky::{EventCursor, PublicKey};
use tokio::sync::watch::Receiver;
use tracing::debug;

use super::{
    invalid_data, invalid_input, run_recording, validate_limit, Missing, WatchOutcome,
    WatcherError, DEFAULT_EVENTS_LIMIT,
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

    /// The handler each event goes to.
    pub fn handler<NewH>(self, handler: NewH) -> KeyStreamWatcherBuilder<NewH>
    where
        NewH: EventHandler<HsStreamEvent, WatcherError> + 'static,
    {
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

impl<H> KeyStreamWatcherBuilder<H>
where
    H: EventHandler<HsStreamEvent, WatcherError> + 'static,
{
    /// Builds the watcher, which stops reading once `shutdown_rx` turns `true`. Fails on an
    /// events limit of zero, an empty or duplicated user list, or a path that does not start
    /// with `/`.
    pub fn build(self, shutdown_rx: Receiver<bool>) -> Result<KeyStreamWatcher, WatcherError> {
        validate_limit(self.events_limit)?;
        validate_users(&self.users)?;
        if !self.path.starts_with('/') {
            return Err(invalid_input("the event-stream path must start with '/'"));
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

struct Config {
    homeserver: PublicKey,
    homeserver_id: String,
    users: Vec<(PublicKey, EventCursor)>,
    handler: Arc<dyn EventHandler<HsStreamEvent, WatcherError>>,
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
/// one batch each per [`Self::run`].
pub struct KeyStreamWatcher {
    config: Arc<Config>,
}

impl KeyStreamWatcher {
    /// Reads each user's events after their cursor, hands them to the handler in order, and
    /// returns each user's cursor to start the next run from.
    ///
    /// A user's stream is checked before any of its events is handled: no more events than
    /// requested, every event the user's own, and cursors that only move forward. A stream
    /// that breaks this, or a handler error, fails the run.
    pub async fn run(&self) -> Result<KeyStreamOutcome, WatcherError> {
        let run = Arc::new(Run {
            config: self.config.clone(),
            outcome: Mutex::new(None),
        });
        run_recording(run, |run: &Run| &run.outcome).await
    }
}

/// One run of a [`KeyStreamWatcher`], as a processor.
struct Run {
    config: Arc<Config>,
    outcome: Mutex<Option<KeyStreamOutcome>>,
}

#[async_trait::async_trait]
impl TEventProcessor<HsStreamEvent, WatcherError> for Run {
    fn instance_name(&self) -> &'static str {
        "KeyStreamWatcher"
    }

    fn homeserver_id(&self) -> &str {
        &self.config.homeserver_id
    }

    async fn handle(&self, event: &HsStreamEvent) -> Result<(), WatcherError> {
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
    async fn read(&self) -> Result<KeyStreamOutcome, WatcherError> {
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
    ) -> Result<WatchOutcome, WatcherError> {
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

fn validate_users(users: &[(PublicKey, EventCursor)]) -> Result<(), WatcherError> {
    if users.is_empty() {
        return Err(invalid_input("a key stream needs at least one user"));
    }
    for (index, (user, _)) in users.iter().enumerate() {
        if users[..index].iter().any(|(earlier, _)| earlier == user) {
            return Err(invalid_input("a key stream cannot list a user twice"));
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
) -> Result<(), WatcherError> {
    if events.len() > limit as usize {
        return Err(invalid_data(format!(
            "the stream of {} returned {} events, more than the {limit} requested",
            user.z32(),
            events.len()
        )));
    }

    let mut floor = requested.id();
    for event in events {
        if event.owner() != user {
            return Err(invalid_data(format!(
                "the stream of {} returned an event of {}",
                user.z32(),
                event.owner().z32()
            )));
        }
        if event.cursor <= floor {
            return Err(invalid_data(format!(
                "the stream of {} returned cursor {}, which does not move past {floor}",
                user.z32(),
                event.cursor
            )));
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
        EventHandler, EventMethod, HsError, HsStreamEvent, KeyEventSource, Watcher, WatcherError,
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
    impl EventHandler<HsStreamEvent, WatcherError> for &'static Recorder {
        async fn handle(&self, event: &HsStreamEvent) -> Result<(), WatcherError> {
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
        for events in [
            vec![event(&alice, 11), event(&alice, 12), event(&alice, 13)],
            vec![event(&alice, 11), event(&mallory, 12)],
            vec![event(&alice, 10)],
            vec![event(&alice, 12), event(&alice, 11)],
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

            assert!(result.is_err());
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
        assert!(build(one(), 0, "/pub/").is_err());
        assert!(build(vec![], 1, "/pub/").is_err());
        assert!(build([one(), one()].concat(), 1, "/pub/").is_err());
        assert!(build(one(), 1, "pub/").is_err());
    }
}
