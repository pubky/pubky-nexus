//! # Nexus Watcher
//!
//! The `nexus-watcher` crate is responsible for monitoring a Pubky homeserver’s `/events` endpoint and processing
//! events into the Nexus databases. It integrates with `nexus-common` to manage database connections (Neo4j and Redis),
//! configuration, logging, metrics, and stack initialization.
//!
//! Key responsibilities include:
//!
//! - Listening to a homeserver’s events stream.
//! - Processing various event types (posts, bookmarks, follows, mutes, tags, user updates, etc.).
//! - Applying retry logic for events that fail to index.
//! - Updating both the graph database and Redis indexes based on incoming events.
//!
//! The crate provides a builder interface via `NexusWatcher::builder()` and supports configuration from files.
//! The main entry point is in `main.rs`, which simply calls the builder’s `run()` method to start the event loop.

mod builder;
pub mod events;
pub mod service;
mod traits;

pub use builder::NexusWatcherBuilder;
pub use traits::{TEventProcessor, TEventProcessorFactory};
