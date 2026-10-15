use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use nexus_common::WatcherConfig;
use tracing::info;

struct BackoffState {
    next_backoff_secs: u64,
    backoff_until: Instant,
}

/// Tracks per-HS failure counts and exponential backoff windows.
///
/// Held by the shared [`HomeserverClient`](super::HomeserverClient). Only the external loop
/// consults and records it, through [`HomeserverClient::health`](super::HomeserverClient::health).
pub struct HomeserverHealth {
    initial_backoff_secs: u64,
    max_backoff_secs: u64,
    state: Mutex<HashMap<String, BackoffState>>,
}

impl HomeserverHealth {
    pub fn new(initial_backoff_secs: u64, max_backoff_secs: u64) -> Self {
        if initial_backoff_secs > max_backoff_secs {
            panic!("Invalid config: initial_backoff_secs > max_backoff_secs");
        }

        Self {
            initial_backoff_secs,
            max_backoff_secs,
            state: Mutex::new(HashMap::new()),
        }
    }

    /// The backoff `config` sets: `initial_backoff_secs` doubling up to `max_backoff_secs`.
    pub fn from_config(config: &WatcherConfig) -> Self {
        Self::new(config.initial_backoff_secs, config.max_backoff_secs)
    }

    fn state(&self) -> std::sync::MutexGuard<'_, HashMap<String, BackoffState>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Returns `true` if the homeserver is currently in a backoff window and should be skipped.
    pub fn should_skip(&self, hs_id: &str) -> bool {
        match self.state().get(hs_id) {
            Some(bs) => Instant::now() < bs.backoff_until,
            None => false,
        }
    }

    /// Resets backoff state for a homeserver after a successful run.
    pub fn record_success(&self, hs_id: &str) {
        self.state().remove(hs_id);
    }

    /// Increments the failure counter and sets the next backoff window.
    ///
    /// Backoff duration: `min(BASE * 2^failures, MAX)` — i.e. 60s, 120s, 240s, … up to 1 hour.
    pub fn record_failure(&self, hs_id: &str) {
        let initial = self.initial_backoff_secs;
        let max = self.max_backoff_secs;
        let mut state = self.state();
        let entry = state.entry(hs_id.to_string()).or_insert(BackoffState {
            next_backoff_secs: initial,
            backoff_until: Instant::now(),
        });

        let backoff_secs = entry.next_backoff_secs;
        entry.backoff_until = Instant::now() + Duration::from_secs(backoff_secs);
        entry.next_backoff_secs = (backoff_secs * 2).min(max);

        info!(homeserver = %hs_id, backoff_secs, "Homeserver backed off");
    }
}

impl Default for HomeserverHealth {
    fn default() -> Self {
        use nexus_common::{DEFAULT_INITIAL_BACKOFF_SECS, DEFAULT_MAX_BACKOFF_SECS};
        Self::new(DEFAULT_INITIAL_BACKOFF_SECS, DEFAULT_MAX_BACKOFF_SECS)
    }
}

#[cfg(test)]
impl HomeserverHealth {
    pub(crate) fn next_backoff_secs_for(&self, hs_id: &str) -> Option<u64> {
        self.state().get(hs_id).map(|bs| bs.next_backoff_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_config_uses_the_configured_backoff() {
        let config = WatcherConfig {
            initial_backoff_secs: 2,
            max_backoff_secs: 16,
            ..WatcherConfig::default()
        };
        let health = HomeserverHealth::from_config(&config);

        health.record_failure("hs1");
        assert_eq!(health.next_backoff_secs_for("hs1"), Some(4));
        for _ in 0..3 {
            health.record_failure("hs1");
        }
        assert_eq!(health.next_backoff_secs_for("hs1"), Some(16));
    }

    #[test]
    fn new_homeserver_is_not_skipped() {
        let health = HomeserverHealth::default();
        assert!(!health.should_skip("hs1"));
    }

    #[test]
    fn skipped_after_failure() {
        let health = HomeserverHealth::default();
        health.record_failure("hs1");
        assert!(health.should_skip("hs1"));
    }

    #[test]
    fn reset_after_success() {
        let health = HomeserverHealth::default();
        health.record_failure("hs1");
        health.record_success("hs1");
        assert!(!health.should_skip("hs1"));
    }

    #[test]
    fn independent_homeservers() {
        let health = HomeserverHealth::default();
        health.record_failure("hs1");
        assert!(!health.should_skip("hs2"));
    }

    #[test]
    fn failures_are_capped() {
        let health = HomeserverHealth::default();
        for _ in 0..20 {
            health.record_failure("hs1");
        }
        // Should not panic or overflow, and should still be skipped
        assert!(health.should_skip("hs1"));
    }

    /// Verifies the backoff duration doubles on each failure and is capped at max_backoff_secs.
    /// `next_backoff_secs` holds the duration for the *next* failure, so after the first failure
    /// it is already 2× the initial value.
    #[test]
    fn backoff_duration_sequence() {
        let health = HomeserverHealth::new(2, 16);

        health.record_failure("hs1");
        assert_eq!(health.next_backoff_secs_for("hs1"), Some(4));

        health.record_failure("hs1");
        assert_eq!(health.next_backoff_secs_for("hs1"), Some(8));

        health.record_failure("hs1");
        assert_eq!(health.next_backoff_secs_for("hs1"), Some(16));

        // Further failures must stay at the cap, never exceed it
        health.record_failure("hs1");
        assert_eq!(health.next_backoff_secs_for("hs1"), Some(16));
    }

    /// Verifies that a successful run fully resets backoff state so the next failure
    /// starts the doubling sequence from the initial duration again, not from wherever
    /// it left off before the success.
    #[test]
    fn success_resets_backoff_to_initial() {
        let health = HomeserverHealth::new(2, 32);

        // Two failures advance next_backoff_secs to 8
        health.record_failure("hs1");
        health.record_failure("hs1");
        assert_eq!(health.next_backoff_secs_for("hs1"), Some(8));

        // Success clears all state
        health.record_success("hs1");
        assert_eq!(health.next_backoff_secs_for("hs1"), None);

        // First failure after reset must use the initial duration, not the previous 8
        health.record_failure("hs1");
        assert_eq!(health.next_backoff_secs_for("hs1"), Some(4)); // 2*2, not 8
    }
}
