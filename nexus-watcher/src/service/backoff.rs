use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tracing::info;

const INITIAL_BACKOFF_SECS: u64 = 60;
const MAX_BACKOFF_SECS: u64 = 3_600;

struct BackoffState {
    next_backoff_secs: u64,
    backoff_until: Instant,
}

/// Tracks per-homeserver failure counts and exponential backoff windows.
///
/// Thread-safe via internal `Mutex`. All state is in-memory and lost on restart.
#[derive(Default)]
pub struct HomeserverBackoff {
    state: Mutex<HashMap<String, BackoffState>>,
}

impl HomeserverBackoff {
    /// Returns `true` if the homeserver is currently in a backoff window and should be skipped.
    pub fn should_skip(&self, hs_id: &str) -> bool {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match state.get(hs_id) {
            Some(bs) => Instant::now() < bs.backoff_until,
            None => false,
        }
    }

    /// Resets backoff state for a homeserver after a successful run.
    pub fn record_success(&self, hs_id: &str) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.remove(hs_id);
    }

    /// Increments the failure counter and sets the next backoff window.
    ///
    /// Backoff duration: `min(BASE * 2^failures, MAX)` — i.e. 60s, 120s, 240s, … up to 1 hour.
    pub fn record_failure(&self, hs_id: &str) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let entry = state.entry(hs_id.to_string()).or_insert(BackoffState {
            next_backoff_secs: INITIAL_BACKOFF_SECS,
            backoff_until: Instant::now(),
        });

        let backoff_secs = entry.next_backoff_secs;
        entry.backoff_until = Instant::now() + Duration::from_secs(backoff_secs);
        entry.next_backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF_SECS);

        info!("Homeserver {hs_id} backed off for {backoff_secs}s");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_homeserver_is_not_skipped() {
        let backoff = HomeserverBackoff::default();
        assert!(!backoff.should_skip("hs1"));
    }

    #[test]
    fn skipped_after_failure() {
        let backoff = HomeserverBackoff::default();
        backoff.record_failure("hs1");
        assert!(backoff.should_skip("hs1"));
    }

    #[test]
    fn reset_after_success() {
        let backoff = HomeserverBackoff::default();
        backoff.record_failure("hs1");
        backoff.record_success("hs1");
        assert!(!backoff.should_skip("hs1"));
    }

    #[test]
    fn independent_homeservers() {
        let backoff = HomeserverBackoff::default();
        backoff.record_failure("hs1");
        assert!(!backoff.should_skip("hs2"));
    }

    #[test]
    fn failures_are_capped() {
        let backoff = HomeserverBackoff::default();
        for _ in 0..20 {
            backoff.record_failure("hs1");
        }
        // Should not panic or overflow, and should still be skipped
        assert!(backoff.should_skip("hs1"));
    }
}
