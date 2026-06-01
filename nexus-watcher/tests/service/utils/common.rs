use crate::utils::MockEventHandler;
use nexus_common::models::event::EventProcessorError;
use nexus_watcher::events::retry::{InMemoryRetryStore, RetryStore};
use nexus_watcher::service::user_hs_resolver::{MockPkdnsResolver, PkdnsHomeserverResolver};
use pubky_app_specs::PubkyId;
use std::sync::{Arc, Mutex};

pub const TEST_USER_ID: &str = "uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko";

pub fn new_in_memory_store() -> Arc<dyn RetryStore> {
    Arc::new(InMemoryRetryStore::new())
}

/// Builds a [`MockPkdnsResolver`] resolving every user to `hs_id` (or to nothing
/// when `None`).
pub fn mock_resolver(hs_id: Option<&str>) -> Arc<dyn PkdnsHomeserverResolver> {
    let result = hs_id.map(|id| PubkyId::try_from(id).expect("valid pubky id"));
    Arc::new(MockPkdnsResolver::new(result))
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
        handled_uris: Arc::new(Mutex::new(Vec::new())),
    }
    .into()
}
