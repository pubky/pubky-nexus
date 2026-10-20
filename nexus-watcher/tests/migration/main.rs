//! Homeserver migration suite.
//!
//! Each test starts its own testnet with three homeservers, the first being
//! nexus's primary (bulk-indexed), and moves users between them the way the
//! client does: sign up on the target, which publishes the record naming it,
//! copy the public tree over, maybe wipe the old homeserver. Users act through
//! their own SDK clients with their own pkarr caches, so nexus learns about a
//! switch only from the network, as in production.
//!
//! The tests drive nexus one step at a time instead of running the periodic
//! tasks: one resolver sweep that looks up one user with a fresh lookup, one
//! per-user read of a homeserver, one bulk poll of the primary. The
//! assertions describe the behaviour nexus should have after a switch, in
//! terms of what is indexed, so they do not depend on how the resolver
//! stores a binding. Correctness must not depend on when nexus notices a
//! switch, so the scenarios vary that moment and check the same end state.
//!
//! Nexus's Pubky client is a process-wide singleton bound to the first testnet
//! created in the process, so every test needs its own process: run the suite
//! with nextest as CI does (`cargo nextest run -p nexus-watcher --test migration`)
//! or one test at a time with `cargo test`. Needs the shared Redis and Neo4j
//! and `TEST_PUBKY_CONNECTION_STRING` like the watcher suite.

mod harness;
mod primary;
mod records;
mod switches;
mod wipe;
