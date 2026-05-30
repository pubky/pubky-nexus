use std::collections::HashMap;

use pubky::PublicKey;
use tokio::sync::Mutex;

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
    pub async fn consume_skip(&self, user_pk: &PublicKey) -> bool {
        let mut map = self.inner.lock().await;
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
    pub async fn record_failure(&self, user_pk: &PublicKey) {
        let mut map = self.inner.lock().await;
        let entry = map.entry(user_pk.clone()).or_default();
        entry.consecutive_failures = (entry.consecutive_failures + 1).min(MAX_USER_NOT_FOUND_SKIPS);
        entry.skips_remaining = entry.consecutive_failures;
    }

    /// Clears any tracked 404 backoff for the user after a successful fetch.
    pub async fn record_success(&self, user_pk: &PublicKey) {
        self.inner.lock().await.remove(user_pk);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_pk() -> PublicKey {
        pubky::Keypair::random().public_key()
    }

    #[tokio::test]
    async fn new_user_is_not_skipped() {
        let backoff = UserNotFoundBackoff::default();
        let pk = random_pk();
        assert!(!backoff.consume_skip(&pk).await);
    }

    #[tokio::test]
    async fn skip_count_grows_and_caps() {
        let backoff = UserNotFoundBackoff::default();
        let pk = random_pk();

        backoff.record_failure(&pk).await;
        assert!(backoff.consume_skip(&pk).await);
        assert!(!backoff.consume_skip(&pk).await);

        backoff.record_failure(&pk).await;
        assert!(backoff.consume_skip(&pk).await);
        assert!(backoff.consume_skip(&pk).await);
        assert!(!backoff.consume_skip(&pk).await);

        for _ in 0..(MAX_USER_NOT_FOUND_SKIPS + 5) {
            backoff.record_failure(&pk).await;
        }
        for _ in 0..MAX_USER_NOT_FOUND_SKIPS {
            assert!(backoff.consume_skip(&pk).await);
        }
        assert!(!backoff.consume_skip(&pk).await);
    }

    #[tokio::test]
    async fn success_resets_and_users_are_independent() {
        let backoff = UserNotFoundBackoff::default();
        let pk1 = random_pk();
        let pk2 = random_pk();

        backoff.record_failure(&pk1).await;
        backoff.record_failure(&pk1).await;
        assert!(backoff.consume_skip(&pk1).await);
        assert!(backoff.consume_skip(&pk1).await);
        assert!(!backoff.consume_skip(&pk1).await);

        backoff.record_success(&pk1).await;
        assert!(!backoff.consume_skip(&pk1).await);

        backoff.record_failure(&pk1).await;
        assert!(backoff.consume_skip(&pk1).await);
        assert!(!backoff.consume_skip(&pk1).await);

        assert!(!backoff.consume_skip(&pk2).await);
    }
}
