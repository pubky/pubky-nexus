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
//! tasks: a recheck of one user (what `PUT /v0/recheck/{user}` does), or one
//! resolver sweep with stale mapping disabled that looks up one user, then
//! one per-user read of a homeserver or one bulk poll of the primary. The
//! assertions describe what is indexed, so they do not depend on how the
//! resolver stores a binding. Correctness must not depend on when nexus
//! notices a switch, so the scenarios vary that moment and check the same
//! end state.
//!
//! Nexus's Pubky client is a process-wide singleton bound to the first testnet
//! created in the process, so every test needs its own process: run the suite
//! with nextest as CI does (`cargo nextest run -p nexus-watcher --test migration`)
//! or one test at a time with `cargo test`. Needs the shared Redis and Neo4j
//! and `TEST_PUBKY_CONNECTION_STRING` like the watcher suite.

mod harness;
mod hazards;
mod primary;
mod records;
mod retry_queue;
mod switches;
mod wipe;
