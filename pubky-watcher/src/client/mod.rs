//! Every call the watcher makes to a homeserver, and every lookup of the PKDNS record a user
//! publishes, goes through [`WatcherClient`], built once and shared by all the watcher's loops.
//! Its capabilities are split into narrow traits, so that a caller, or a test, depends only on
//! the one it uses:
//!
//! - [`HomeserverEventSource`]: a homeserver's `/events/` feed
//! - [`KeyEventSource`]: a user's `/events-stream`
//! - [`ResourceReader`]: a `PUT` event's resource or blob, under a size cap
//! - [`HomeserverResolver`]: a user's PKDNS record
//!
//! The module depends on no Nexus type: homeservers and users are [`pubky::PublicKey`]s,
//! size caps are plain values, and it records no metrics.

mod error;
mod event;
mod fetch;
mod resolver;
mod watcher_client;

pub use error::{ClientError, HsError, HsErrorKind, HsOp};
pub use event::EventMethod;
pub use resolver::{published_homeserver, HomeserverResolver, ResolveError};
pub use watcher_client::{
    HomeserverEventSource, HsStreamEvent, KeyEventSource, ResourceReader, WatcherClient,
    DEFAULT_MAX_FILE_SIZE, DEFAULT_REQUEST_TIMEOUT,
};
