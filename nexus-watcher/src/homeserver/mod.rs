//! Every HTTP call the watcher makes to a homeserver goes through [`HomeserverClient`],
//! built once and shared by all the watcher's loops.

mod client;
mod error;
mod fetch;
mod health;

pub use client::{Deadline, HomeserverClient, HsStreamEvent, PubkyHomeserverClient};
pub use error::{HsError, HsErrorKind, HsOp};
pub use health::HomeserverHealth;
