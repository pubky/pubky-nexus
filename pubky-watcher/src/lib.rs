//! # Pubky Watcher
//!
//! Reads events from Pubky homeservers, for applications that index them: search engines,
//! moderation tools, archives, application-specific indexes.
//!
//! The crate owns the generic mechanics: the homeserver client and its capability traits,
//! framing a homeserver's `/events/` body, parsing its lines, running a processor and waiting
//! for it, running processors across many homeservers, and the hooks through which an
//! application plugs in its own handling and retry policy. The application owns what an event
//! means, where it is stored, where cursors are kept, and its retry and backoff policy.
//!
//! - [`WatcherClient`] makes every homeserver call, split into narrow capability traits:
//!   [`HomeserverEventSource`], [`KeyEventSource`], [`ResourceReader`] and
//!   [`HomeserverResolver`]. A failure is an [`HsError`].
//! - [`TEventProcessor`] is one run over one homeserver, generic over the application's
//!   event and error types; [`TEventProcessorRunner`] runs one per homeserver.
//! - Retry is opt-in through [`RetryableError`], [`EventRetryScheduler`] and
//!   [`dispatch_retryable_error`].
//!
//! Homeservers and users are [`pubky::PublicKey`]s in the client. The crate brings no
//! database, metrics or application config with it.

mod client;
mod events;
mod processor;
mod runner;
mod traits;
mod watcher;

pub use client::{
    published_homeserver, ClientError, EventMethod, HomeserverEventSource, HomeserverResolver,
    HsError, HsErrorKind, HsOp, HsStreamEvent, KeyEventSource, ResolveError, ResourceReader,
    WatcherClient, DEFAULT_MAX_FILE_SIZE, DEFAULT_REQUEST_TIMEOUT,
};
pub use events::{EventBatch, HomeserverEvent, CURSOR_PREFIX};
pub use processor::{
    dispatch_retryable_error, run_on_own_task, RunError, RunOutcome, TEventProcessor,
    SLOW_RUN_THRESHOLD,
};
pub use runner::{
    status_from_run_result, ProcessedStats, ProcessorRunStats, ProcessorRunStatus,
    RunAllProcessorsStats, TEventProcessorRunner,
};
pub use traits::{
    EventHandler, EventRetryScheduler, LineParseOutcome, ParseFromLine, RetryableError,
};
pub use watcher::{
    ConfigError, HomeserverWatcher, HomeserverWatcherBuilder, KeyStreamOutcome, KeyStreamWatcher,
    KeyStreamWatcherBuilder, Missing, ProtocolError, WatchOutcome, Watcher, WatcherError,
    DEFAULT_EVENTS_LIMIT, DEFAULT_KEY_STREAM_PATH,
};
