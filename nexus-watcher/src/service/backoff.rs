use std::collections::HashMap;
use std::time::{Duration, Instant};

use tracing::info;

struct BackoffState {
    next_backoff_secs: u64,
    backoff_until: Instant,
}

/// Tracks per-homeserver failure counts and exponential backoff windows.
pub struct HomeserverBackoff {
    initial_backoff_secs: u64,
    max_backoff_secs: u64,
    state: HashMap<String, BackoffState>,
}

impl HomeserverBackoff {
    /// Creates a new `HomeserverBackoff` with the given initial and maximum backoff durations.
    pub fn new(initial_backoff_secs: u64, max_backoff_secs: u64) -> Self {
        Self {
            initial_backoff_secs,
            max_backoff_secs,
            state: HashMap::new(),
        }
    }

    /// Returns `true` if the homeserver is currently in a backoff window and should be skipped.
    pub fn should_skip(&self, hs_id: &str) -> bool {
        match self.state.get(hs_id) {
            Some(bs) => Instant::now() < bs.backoff_until,
            None => false,
        }
    }

    /// Resets backoff state for a homeserver after a successful run.
    pub fn record_success(&mut self, hs_id: &str) {
        self.state.remove(hs_id);
    }

    /// Increments the failure counter and sets the next backoff window.
    ///
    /// Backoff duration: `min(BASE * 2^failures, MAX)` — i.e. 60s, 120s, 240s, … up to 1 hour.
    pub fn record_failure(&mut self, hs_id: &str) {
        let initial = self.initial_backoff_secs;
        let max = self.max_backoff_secs;
        let entry = self.state.entry(hs_id.to_string()).or_insert(BackoffState {
            next_backoff_secs: initial,
            backoff_until: Instant::now(),
        });

        let backoff_secs = entry.next_backoff_secs;
        entry.backoff_until = Instant::now() + Duration::from_secs(backoff_secs);
        entry.next_backoff_secs = (backoff_secs * 2).min(max);

        info!("Homeserver {hs_id} backed off for {backoff_secs}s");
    }
}

impl Default for HomeserverBackoff {
    fn default() -> Self {
        use nexus_common::{DEFAULT_INITIAL_BACKOFF_SECS, DEFAULT_MAX_BACKOFF_SECS};
        Self::new(DEFAULT_INITIAL_BACKOFF_SECS, DEFAULT_MAX_BACKOFF_SECS)
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
        let mut backoff = HomeserverBackoff::default();
        backoff.record_failure("hs1");
        assert!(backoff.should_skip("hs1"));
    }

    #[test]
    fn reset_after_success() {
        let mut backoff = HomeserverBackoff::default();
        backoff.record_failure("hs1");
        backoff.record_success("hs1");
        assert!(!backoff.should_skip("hs1"));
    }

    #[test]
    fn independent_homeservers() {
        let mut backoff = HomeserverBackoff::default();
        backoff.record_failure("hs1");
        assert!(!backoff.should_skip("hs2"));
    }

    #[test]
    fn failures_are_capped() {
        let mut backoff = HomeserverBackoff::default();
        for _ in 0..20 {
            backoff.record_failure("hs1");
        }
        // Should not panic or overflow, and should still be skipped
        assert!(backoff.should_skip("hs1"));
    }
}
