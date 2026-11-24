//! Watchers of one homeserver, for an application that needs no more than a handler and a
//! place to keep cursors.
//!
//! [`Watcher::homeserver`] reads a homeserver's `/events/` feed, which covers every user it
//! hosts under one cursor. [`Watcher::key_stream`] reads a known set of users'
//! `/events-stream`s on one homeserver, with one cursor each. Each run returns the cursors
//! that are safe to persist; where they are kept is the application's choice.

#![allow(
    clippy::result_large_err,
    clippy::large_enum_variant,
    reason = "the errors carry the keys involved (a `PublicKey` is about 200 bytes); a run fails at most once"
)]

mod homeserver;
mod key_stream;

use std::fmt::{Debug, Display};
use std::future::Future;

use pubky::{EventCursor, PublicKey};
use thiserror::Error;

use crate::processor::{wait_on_own_task, Panicked, SLOW_RUN_THRESHOLD};
use crate::{HomeserverEventSource, HsError, KeyEventSource};

pub use homeserver::{HomeserverWatcher, HomeserverWatcherBuilder};
pub use key_stream::{
    KeyStreamOutcome, KeyStreamWatcher, KeyStreamWatcherBuilder, DEFAULT_KEY_STREAM_PATH,
};

/// The events a run requests when the builder sets no limit.
/// Set another with [`HomeserverWatcherBuilder::events_limit`] or
/// [`KeyStreamWatcherBuilder::events_limit`].
pub const DEFAULT_EVENTS_LIMIT: u16 = 100;

/// Why a builder refused to build its watcher: see [`HomeserverWatcherBuilder::build`] and
/// [`KeyStreamWatcherBuilder::build`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// The events limit ([`HomeserverWatcherBuilder::events_limit`],
    /// [`KeyStreamWatcherBuilder::events_limit`]) is zero, so a run could never read an event.
    #[error("the events limit must be greater than zero")]
    ZeroEventsLimit,
    /// [`Watcher::key_stream`] was given no users to read.
    #[error("a key stream needs at least one user")]
    NoUsers,
    /// [`Watcher::key_stream`] was given `user` more than once, each with its own cursor.
    #[error("a key stream cannot list a user twice")]
    DuplicateUser {
        /// The user listed twice.
        user: PublicKey,
    },
    /// The key stream's path ([`KeyStreamWatcherBuilder::path`]) does not start with `/`.
    #[error("the event-stream path must start with '/'")]
    InvalidPath {
        /// The path given to [`KeyStreamWatcherBuilder::path`].
        path: String,
    },
}

/// A homeserver answer that breaks the events protocol. The run fails before any event of
/// the answer is handled, so the cursor it started from is still safe to persist.
/// A failed run carries it as [`WatcherError::Protocol`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ProtocolError {
    /// A `/events/` batch had events but no `cursor:` line.
    /// Checked by [`HomeserverWatcher::run`].
    #[error("the homeserver sent events without a cursor line")]
    MissingCursor,
    /// A `/events/` batch's cursor is not a number.
    /// Checked by [`HomeserverWatcher::run`].
    #[error("the homeserver sent a cursor that is no number: {raw:?}")]
    MalformedCursor {
        /// The cursor as the homeserver sent it.
        /// Without its [`CURSOR_PREFIX`](crate::CURSOR_PREFIX).
        raw: String,
    },
    /// Events came with a cursor that does not move past the one they had to: the one the
    /// run asked from, or on a key stream, the previous event's.
    /// Checked by [`HomeserverWatcher::run`] and [`KeyStreamWatcher::run`].
    #[error("{}", cursor_not_advanced(.user.as_ref(), *.requested, *.got))]
    CursorNotAdvanced {
        /// The user whose stream it was, on a [`KeyStreamWatcher`]; `None` on a
        /// [`HomeserverWatcher`].
        user: Option<PublicKey>,
        /// The cursor the events had to move past.
        /// On a [`KeyStreamWatcher`], the previous event's cursor after the first event.
        requested: u64,
        /// The cursor the homeserver sent.
        /// The batch's cursor on a [`HomeserverWatcher`], the event's on a [`KeyStreamWatcher`].
        got: u64,
    },
    /// A `/events/` batch had no events but moved the cursor, which would skip whatever lies
    /// between.
    /// Checked by [`HomeserverWatcher::run`].
    #[error("the homeserver sent no events but moved the cursor from {requested} to {got}")]
    CursorMovedWithoutEvents {
        /// The cursor the run asked from.
        requested: u64,
        /// The cursor the homeserver sent.
        /// The batch's cursor line ([`EventBatch::cursor`](crate::EventBatch::cursor)).
        got: u64,
    },
    /// A user's stream held more events than the run asked for.
    /// Checked by [`KeyStreamWatcher::run`].
    #[error("the stream of {} returned {got} events, more than the {limit} requested", .user.z32())]
    TooManyEvents {
        /// The user whose stream it was.
        user: PublicKey,
        /// The events the stream held.
        got: usize,
        /// The events the run asked for.
        /// Set by [`KeyStreamWatcherBuilder::events_limit`].
        limit: u16,
    },
    /// A user's stream held an event of another user, which would let one user's stream
    /// inject another's events.
    /// Checked by [`KeyStreamWatcher::run`].
    #[error("the stream of {} returned an event of {}", .user.z32(), .owner.z32())]
    ForeignEvent {
        /// The user whose stream it was.
        user: PublicKey,
        /// The owner of the event ([`HsStreamEvent::owner`](crate::HsStreamEvent::owner)).
        owner: PublicKey,
    },
}

fn cursor_not_advanced(user: Option<&PublicKey>, requested: u64, got: u64) -> String {
    match user {
        None => format!(
            "the homeserver sent events with cursor {got}, which does not move past {requested}"
        ),
        Some(user) => format!(
            "the stream of {} returned cursor {got}, which does not move past {requested}",
            user.z32()
        ),
    }
}

/// Why a run of a [`HomeserverWatcher`] or a [`KeyStreamWatcher`] failed. `Err` is the
/// handler's own error, given back as it was returned.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WatcherError<Err> {
    /// A homeserver call failed, with an [`HsError`].
    #[error(transparent)]
    Homeserver(#[from] HsError),
    /// The homeserver's answer breaks the events protocol, as a [`ProtocolError`] says.
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    /// The handler failed on an event; the events before it were handled.
    /// `Err` is what the [`EventHandler`](crate::EventHandler) returned.
    ///
    /// The error is displayed but is not this variant's `source()`: `Err` need not implement
    /// [`std::error::Error`], so a `Box<dyn Error>` or a `String` can be one. A reporter that
    /// walks a `WatcherError`'s chain stops at the handler's message, so match this variant to
    /// reach the handler's error and its causes.
    #[error("{0}")]
    Handler(Err),
    /// The run panicked.
    /// As [`RunError::Panicked`](crate::RunError::Panicked) is for a processor.
    #[error("the watcher run panicked")]
    Panicked,
}

/// A builder whose handler is not set yet.
/// [`HomeserverWatcherBuilder::handler`] and [`KeyStreamWatcherBuilder::handler`]
/// replace it.
#[derive(Debug, Default)]
pub struct Missing;

/// How a run of a [`HomeserverWatcher`] ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchOutcome {
    /// The cursor to start the next run from, safe to persist: every event before it was
    /// handled.
    /// Pass it to the next [`HomeserverWatcher::run`].
    pub cursor: EventCursor,
    /// The events handed to the handler.
    /// Each went to the handler's [`EventHandler::handle`](crate::EventHandler::handle).
    pub processed_events: usize,
    /// Whether the run handled everything it read. A run stopped by the shutdown signal given
    /// to [`HomeserverWatcherBuilder::build`] returns `false`, with the cursor it started from,
    /// so the next run reads the batch again.
    pub completed: bool,
}

impl WatchOutcome {
    fn completed(cursor: EventCursor, processed_events: usize) -> Self {
        Self {
            cursor,
            processed_events,
            completed: true,
        }
    }

    fn interrupted(cursor: EventCursor, processed_events: usize) -> Self {
        Self {
            cursor,
            processed_events,
            completed: false,
        }
    }
}

/// Picks what to watch on one homeserver: [`Watcher::homeserver`] reads its `/events/` feed,
/// [`Watcher::key_stream`] a known set of users' streams.
///
/// ```ignore
/// let watcher = Watcher::homeserver(WatcherClient::mainnet()?, homeserver)
///     .handler(MyHandler)
///     .build(shutdown_rx)?;
/// let outcome = watcher.run(cursor).await?;
/// // persist outcome.cursor
/// ```
pub struct Watcher;

impl Watcher {
    /// Watches `homeserver`'s `/events/` feed, which covers every user it hosts under one
    /// cursor. The handler gets each [`HomeserverEvent`](crate::HomeserverEvent).
    pub fn homeserver<C>(client: C, homeserver: PublicKey) -> HomeserverWatcherBuilder<Missing>
    where
        C: HomeserverEventSource,
    {
        HomeserverWatcherBuilder::new(client, homeserver)
    }

    /// Watches the `/events-stream` of each of `users` on `homeserver`, from the cursor given
    /// with it. The handler gets each [`HsStreamEvent`](crate::HsStreamEvent).
    pub fn key_stream<C>(
        client: C,
        homeserver: PublicKey,
        users: Vec<(PublicKey, EventCursor)>,
    ) -> KeyStreamWatcherBuilder<Missing>
    where
        C: KeyEventSource,
    {
        KeyStreamWatcherBuilder::new(client, homeserver, users)
    }
}

/// Runs `read`, a run of the watcher `instance_name` on `homeserver`, on its own task, as
/// [`run_on_own_task`](crate::run_on_own_task) does a processor's, and returns its outcome. A
/// panic is [`WatcherError::Panicked`].
async fn run_watcher<T, Err>(
    instance_name: &'static str,
    homeserver: &str,
    read: impl Future<Output = Result<T, WatcherError<Err>>> + Send + 'static,
) -> Result<T, WatcherError<Err>>
where
    T: Send + 'static,
    Err: Display + Debug + Send + 'static,
{
    match wait_on_own_task(instance_name, Some(homeserver), SLOW_RUN_THRESHOLD, read).await {
        Ok(result) => result,
        Err(Panicked) => Err(WatcherError::Panicked),
    }
}

fn validate_limit(limit: u16) -> Result<(), ConfigError> {
    if limit == 0 {
        return Err(ConfigError::ZeroEventsLimit);
    }
    Ok(())
}
