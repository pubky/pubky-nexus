use crate::utils::MockEventHandler;
use nexus_common::models::event::EventProcessorError;
use nexus_watcher::events::retry::{InMemoryRetryStore, RetryStore};
use std::sync::{Arc, Mutex};

pub const TEST_USER_ID: &str = "uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko";

pub fn new_in_memory_store() -> Arc<dyn RetryStore> {
    Arc::new(InMemoryRetryStore::new())
}

/// Create a mock event handler with an invocation counter.
///
/// The returned handler tracks how many times `handle()` was called via
/// its `handle_count` field. Wrap it in `Arc` before passing to processors.
pub fn create_mock_handler(
    result: Result<(), EventProcessorError>,
    target_substring: Option<&str>,
) -> Arc<MockEventHandler> {
    MockEventHandler {
        result,
        target_uri_substring: target_substring.map(str::to_string),
        handle_count: Arc::new(Mutex::new(0)),
    }
    .into()
}
