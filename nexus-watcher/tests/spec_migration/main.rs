//! Spec migration suite: a user moving from `pubky.app` (`v0`) to `social/v1`, end to end.
//!
//! Each test starts its own testnet with two homeservers, the first being nexus's primary
//! (bulk-indexed), the second external (read per user). Users act through their own SDK
//! sessions. A migration is what the app does: it runs the migration engine over the user's
//! tree on the homeserver, which copies every `pub/pubky.app/` object to its `social/v1`
//! form through the crate's own transforms and writes the `social/v1` profile last among the
//! public objects ([`migrator`]). Nexus sees nothing but the homeserver's event stream: the
//! copies, which it ignores for a `v0` user, then the profile PUT, which flips the user and
//! rewrites what it holds for them from the graph.
//!
//! The tests drive nexus one step at a time instead of running the periodic tasks: one poll of
//! the primary, one per-user read of the external homeserver, one pass of the retry processor.
//! The assertions describe what nexus serves (profiles, posts, counts, tags, streams,
//! notifications), so they hold whatever the flip does inside.
//!
//! The epoch gate and each step of the flip are tested in the event processor suite. This one
//! covers what only a whole migration shows: the flip of a user with a social graph around
//! them ([`flip`]), the engine's copies reaching nexus after the flip ([`copies`]), and users of
//! both epochs reaching each other ([`mixed`]). A scenario that fails on purpose is
//! `#[ignore]`d with the reason; run it with `--run-ignored only`.
//!
//! Nexus's Pubky client is a process-wide singleton bound to the first testnet created in the
//! process, so every test needs its own process: run the suite with nextest as CI does
//! (`cargo nextest run -p nexus-watcher --test spec_migration`) or one test at a time with
//! `cargo test`. Needs Redis, Neo4j and `TEST_PUBKY_CONNECTION_STRING` like the watcher suite;
//! every test makes its own users, so the mock data does not matter.

mod copies;
mod fixtures;
mod flip;
mod harness;
mod migrator;
mod mixed;
