//! Every call the watcher makes to a homeserver goes through [`HomeserverClient`], and every
//! lookup of the PKDNS record a user publishes through [`HomeserverResolver`]. Both are
//! built once and shared by all the watcher's loops.
//!
//! The module depends on no Nexus type: homeservers and users are [`pubky::PublicKey`]s,
//! size caps are plain values, and it records no metrics.

mod client;
mod error;
mod event;
mod fetch;
mod resolver;

pub use client::{HomeserverClient, HsStreamEvent, PubkyHomeserverClient};
pub use error::{HsError, HsErrorKind, HsOp};
pub use event::EventMethod;
pub use resolver::{published_homeserver, HomeserverResolver};
