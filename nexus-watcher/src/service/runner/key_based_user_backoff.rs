use std::collections::HashMap;
use std::sync::Mutex;

use pubky::PublicKey;

/// Upper bound on how many consecutive runs a user can be skipped after repeated 404s.
const MAX_USER_NOT_FOUND_SKIPS: u32 = 10;

/// Per-user backoff state for users whose event fetch returns HTTP 404.
///
/// Tracks consecutive 404s per user (keyed by public key) and derives how many
/// subsequent runs that user should be skipped: the 1st 404 skips the user once,
/// the 2nd skips twice, and so on, capped at `MAX_USER_NOT_FOUND_SKIPS`.
/// A successful fetch clears the user's state.
#[derive(Default)]
pub struct UserNotFoundBackoff {
    inner: Mutex<HashMap<PublicKey, BackoffEntry>>,
}

#[derive(Default)]
struct BackoffEntry {
    /// Number of consecutive 404s observed (capped at [`MAX_USER_NOT_FOUND_SKIPS`]).
    consecutive_failures: u32,
    /// Remaining runs to skip before re-attempting this user.
    skips_remaining: u32,
}

impl UserNotFoundBackoff {
    /// Consumes one unit of the user's pending skip budget, returning `true` if
    /// the user should be skipped this run.
    pub fn consume_skip(&self, user_pk: &PublicKey) -> bool {
        let mut map = self.inner.lock().expect("UserNotFoundBackoff poisoned");
        match map.get_mut(user_pk) {
            Some(entry) if entry.skips_remaining > 0 => {
                entry.skips_remaining -= 1;
                true
            }
            _ => false,
        }
    }

    /// Records a 404 for the user, increasing the number of runs it will be
    /// skipped on subsequent runs (capped at `MAX_USER_NOT_FOUND_SKIPS`).
    pub fn record_failure(&self, user_pk: &PublicKey) {
        let mut map = self.inner.lock().expect("UserNotFoundBackoff poisoned");
        let entry = map.entry(user_pk.clone()).or_default();
        entry.consecutive_failures = (entry.consecutive_failures + 1).min(MAX_USER_NOT_FOUND_SKIPS);
        entry.skips_remaining = entry.consecutive_failures;
    }

    /// Clears any tracked 404 backoff for the user after a successful fetch.
    pub fn clear(&self, user_pk: &PublicKey) {
        self.inner
            .lock()
            .expect("UserNotFoundBackoff poisoned")
            .remove(user_pk);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_pk() -> PublicKey {
        pubky::Keypair::random().public_key()
    }

    #[test]
    fn new_user_is_not_skipped() {
        let backoff = UserNotFoundBackoff::default();
        let pk = random_pk();
        assert!(!backoff.consume_skip(&pk));
    }

    #[test]
    fn skipped_after_first_not_found() {
        let backoff = UserNotFoundBackoff::default();
        let pk = random_pk();
        backoff.record_failure(&pk);
        assert!(backoff.consume_skip(&pk));
        assert!(!backoff.consume_skip(&pk));
    }

    #[test]
    fn skip_count_increases_with_consecutive_not_found() {
        let backoff = UserNotFoundBackoff::default();
        let pk = random_pk();

        backoff.record_failure(&pk);
        assert!(backoff.consume_skip(&pk));
        assert!(!backoff.consume_skip(&pk));

        backoff.record_failure(&pk);
        assert!(backoff.consume_skip(&pk));
        assert!(backoff.consume_skip(&pk));
        assert!(!backoff.consume_skip(&pk));

        backoff.record_failure(&pk);
        assert!(backoff.consume_skip(&pk));
        assert!(backoff.consume_skip(&pk));
        assert!(backoff.consume_skip(&pk));
        assert!(!backoff.consume_skip(&pk));
    }

    #[test]
    fn skip_count_capped_at_max() {
        let backoff = UserNotFoundBackoff::default();
        let pk = random_pk();

        for _ in 0..(MAX_USER_NOT_FOUND_SKIPS + 5) {
            backoff.record_failure(&pk);
        }

        for _ in 0..MAX_USER_NOT_FOUND_SKIPS {
            assert!(backoff.consume_skip(&pk));
        }
        assert!(!backoff.consume_skip(&pk));
    }

    #[test]
    fn clear_resets_backoff() {
        let backoff = UserNotFoundBackoff::default();
        let pk = random_pk();

        backoff.record_failure(&pk);
        backoff.record_failure(&pk);
        backoff.clear(&pk);

        assert!(!backoff.consume_skip(&pk));
    }

    #[test]
    fn clear_then_not_found_starts_fresh() {
        let backoff = UserNotFoundBackoff::default();
        let pk = random_pk();

        backoff.record_failure(&pk);
        backoff.record_failure(&pk);
        backoff.clear(&pk);

        backoff.record_failure(&pk);
        assert!(backoff.consume_skip(&pk));
        assert!(!backoff.consume_skip(&pk));
    }

    #[test]
    fn independent_users() {
        let backoff = UserNotFoundBackoff::default();
        let pk1 = random_pk();
        let pk2 = random_pk();

        backoff.record_failure(&pk1);

        assert!(backoff.consume_skip(&pk1));
        assert!(!backoff.consume_skip(&pk2));
    }

    #[test]
    fn multiple_not_found_accumulates_skips() {
        let backoff = UserNotFoundBackoff::default();
        let pk = random_pk();

        backoff.record_failure(&pk);
        backoff.record_failure(&pk);
        backoff.record_failure(&pk);

        let mut skips = 0;
        while backoff.consume_skip(&pk) {
            skips += 1;
        }
        assert_eq!(skips, 3);
    }
}
