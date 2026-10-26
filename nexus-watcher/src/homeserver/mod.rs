//! Every call the watcher makes to a homeserver, and every lookup of the homeserver a user
//! publishes, goes through [`HomeserverClient`], built once and shared by all the watcher's
//! loops.

mod client;
mod error;
pub(crate) mod fetch;

pub use client::{HomeserverClient, HsStreamEvent, PubkyHomeserverClient};
pub use error::{HsError, HsErrorKind, HsOp};
