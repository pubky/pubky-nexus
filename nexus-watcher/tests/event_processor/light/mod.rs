//! Watcher behaviour in light mode (`mode = "light"`).
//!
//! Each test sets up a light stack with [`WatcherTest::setup_light`], so these tests rely on
//! nextest running every test in its own process.
//!
//! [`WatcherTest::setup_light`]: crate::event_processor::utils::watcher::WatcherTest::setup_light

mod files;
mod homeserver;
mod posts;
mod users;
