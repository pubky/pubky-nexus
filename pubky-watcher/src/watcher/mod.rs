//! Watchers of one homeserver, for an application that needs no more than a handler and a
//! place to keep cursors.
//!
//! [`Watcher::homeserver`] reads a homeserver's `/events/` feed, which covers every user it
//! hosts under one cursor. [`Watcher::key_stream`] reads a known set of users'
//! `/events-stream`s on one homeserver, with one cursor each. Each run returns the cursors
//! that are safe to persist; where they are kept is the application's choice.

mod homeserver;
mod key_stream;

use std::io::{Error, ErrorKind};
use std::sync::{Arc, Mutex};

use pubky::{EventCursor, PublicKey};

use crate::processor::{RunError, TEventProcessor};
use crate::{HomeserverEventSource, KeyEventSource};

pub use homeserver::{HomeserverWatcher, HomeserverWatcherBuilder};
pub use key_stream::{
    KeyStreamOutcome, KeyStreamWatcher, KeyStreamWatcherBuilder, DEFAULT_KEY_STREAM_PATH,
};

/// The events a run requests when the builder sets no limit.
pub const DEFAULT_EVENTS_LIMIT: u16 = 100;

/// Why a watcher could not be built, or a run failed: a misconfiguration, a homeserver call
/// that failed ([`HsError`](crate::HsError)), a homeserver answer that breaks the protocol,
/// or the handler's error.
pub type WatcherError = Box<dyn std::error::Error + Send + Sync>;

/// A builder whose handler is not set yet.
#[derive(Debug, Default)]
pub struct Missing;

/// How a run of a [`HomeserverWatcher`] ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchOutcome {
    /// The cursor to start the next run from, safe to persist: every event before it was
    /// handled.
    pub cursor: EventCursor,
    /// The events handed to the handler.
    pub processed_events: usize,
    /// Whether the run handled everything it read. A run stopped by shutdown returns `false`,
    /// with the cursor it started from, so the next run reads the batch again.
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

/// Picks what to watch on one homeserver.
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

/// Runs `processor` through [`TEventProcessor::run`], so the run is waited for on its own
/// task and a panic is an error, and returns the outcome it recorded in `slot`.
async fn run_recording<E, P, O>(
    processor: Arc<P>,
    slot: impl Fn(&P) -> &Mutex<Option<O>>,
) -> Result<O, WatcherError>
where
    E: Send + Sync + 'static,
    P: TEventProcessor<E, WatcherError>,
{
    processor.clone().run().await.map_err(|error| match error {
        RunError::Internal(error) => error,
        RunError::Panicked => "the watcher run panicked".into(),
        RunError::BudgetSpent => "the watcher run stopped at a budget".into(),
    })?;
    let outcome = slot(&processor)
        .lock()
        .map_err(|_| "the watcher run's outcome is poisoned")?
        .take();
    outcome.ok_or_else(|| "the watcher run recorded no outcome".into())
}

fn validate_limit(limit: u16) -> Result<(), WatcherError> {
    if limit == 0 {
        return Err(invalid_input("the events limit must be greater than zero"));
    }
    Ok(())
}

fn invalid_input(message: &'static str) -> WatcherError {
    Error::new(ErrorKind::InvalidInput, message).into()
}

fn invalid_data(message: String) -> WatcherError {
    Error::new(ErrorKind::InvalidData, message).into()
}
