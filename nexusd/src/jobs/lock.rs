use async_trait::async_trait;
use nexus_common::db::{release_lock, try_acquire_lock, RedisError};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::error::{LockError, LockResult};

/// The one knob: how long a run may take. Callers abandon `run()` at this
/// wall-clock deadline, and the lease is sized from it, so a run can't outlive
/// its lease and overlap another process's.
pub(super) const MAX_RUN: Duration = Duration::from_secs(3600);

/// Lease slack past [`MAX_RUN`], covering the acquire-to-run gap and the release
/// round-trip. Also the crash backstop: after a hard kill the slot frees itself
/// this long after the deadline.
const LEASE_MARGIN: Duration = Duration::from_secs(60);

/// Lease TTL, sized from the deadline it has to outlast.
const LOCK_TTL_SECS: u64 = MAX_RUN.as_secs() + LEASE_MARGIN.as_secs();

/// Cross-process mutual exclusion for a job's runs (the scheduler already
/// serializes within one process). Injected so the scheduler is testable
/// without Redis.
#[async_trait]
pub trait RunLock: Send + Sync {
    /// Mints a fresh token (no I/O), so a [`LockGuard`] can be armed before
    /// `acquire` — a run cancelled mid-acquire then still releases.
    fn new_token(&self) -> String;

    /// Tries to claim the run slot for `job` under `token`. `Ok(false)` when
    /// another run already holds it.
    async fn acquire(&self, job: &str, token: &str) -> LockResult<bool>;

    /// Releases the slot, but only if it's still held by `token`.
    async fn unlock(&self, job: &str, token: &str) -> LockResult<()>;
}

impl From<RedisError> for LockError {
    fn from(e: RedisError) -> Self {
        Self(Box::new(e))
    }
}

/// Redis-backed [`RunLock`] used in production. Construct once per process (via
/// [`new`](Self::new)) and share the `Arc`: the token counter is per-instance,
/// so a second instance would restart it and could mint a colliding token.
pub struct RedisRunLock {
    // pid can be reused after a process exits, so mix in the start time to keep
    // tokens distinct across pid reuse (a cross-process concern).
    seed: u128,
    counter: AtomicU64,
}

impl RedisRunLock {
    pub fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        Self {
            seed,
            counter: AtomicU64::new(0),
        }
    }
}

impl Default for RedisRunLock {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RunLock for RedisRunLock {
    /// A token unique per acquisition within this process: `<pid>-<seed>-<counter>`.
    fn new_token(&self) -> String {
        format!(
            "{}-{}-{}",
            std::process::id(),
            self.seed,
            self.counter.fetch_add(1, Ordering::Relaxed),
        )
    }

    async fn acquire(&self, job: &str, token: &str) -> LockResult<bool> {
        Ok(try_acquire_lock(&key(job), token, LOCK_TTL_SECS).await?)
    }

    async fn unlock(&self, job: &str, token: &str) -> LockResult<()> {
        Ok(release_lock(&key(job), token).await?)
    }
}

fn key(job: &str) -> String {
    format!("lock:job:{job}")
}

/// The outcome of [`acquire`].
pub(super) enum Acquired {
    /// Taken; release through the guard.
    Taken(LockGuard),
    /// Another run holds it. Nothing to release.
    Held,
    /// The backend failed. Anything we might have taken is already released.
    Failed(LockError),
}

/// Takes `job`'s run slot, arming the guard *before* the acquire so a lost SET
/// reply (or a cancel mid-acquire) still releases; the compare-and-delete no-ops
/// if our token was never stored.
///
/// One `job` feeds both the acquire and the guard, so a run can't release a key
/// it never acquired. On backend error the guard is released inline rather than
/// left to Drop: callers may return straight into process exit, where a
/// Drop-spawned unlock never gets polled.
pub(super) async fn acquire(job: &'static str, lock: &Arc<dyn RunLock>) -> Acquired {
    let token = lock.new_token();
    let guard = LockGuard::new(job, token.clone(), Arc::clone(lock));

    match lock.acquire(job, &token).await {
        Ok(true) => Acquired::Taken(guard),
        Ok(false) => {
            guard.disarm();
            Acquired::Held
        }
        Err(e) => {
            guard.release().await;
            Acquired::Failed(e)
        }
    }
}

/// RAII lock release. Prefer `release()`, which awaits the unlock; Drop is the
/// backstop for guards dropped while still armed (a panic in `run()`, or the
/// whole future being dropped).
///
/// Drop releases fire-and-forget via `tokio::spawn`, so it must be dropped in a
/// runtime context, and only frees the lock if that runtime outlives the spawn —
/// otherwise the TTL is the fallback. Don't rely on it where the process may
/// exit right after (see [`acquire`]).
pub struct LockGuard {
    job: &'static str,
    token: Option<String>,
    lock: Arc<dyn RunLock>,
}

impl LockGuard {
    pub(super) fn new(job: &'static str, token: String, lock: Arc<dyn RunLock>) -> Self {
        Self {
            job,
            token: Some(token),
            lock,
        }
    }

    /// Forget the lock without releasing it — for the path where the guard was
    /// armed before the acquire but the lock wasn't actually taken. After this,
    /// Drop is a no-op.
    pub(super) fn disarm(mut self) {
        self.token = None;
    }

    /// Releases the lock, awaiting the result so unlock errors log
    /// synchronously. After this, Drop is a no-op.
    pub async fn release(mut self) {
        let Some(token) = self.token.take() else {
            return;
        };
        if let Err(e) = self.lock.unlock(self.job, &token).await {
            tracing::warn!(
                job = self.job,
                "Could not release run lock (will expire via TTL): {e}"
            );
        }
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let Some(token) = self.token.take() else {
            return;
        };
        let lock = self.lock.clone();
        let job = self.job;
        // Fire-and-forget: reached whenever a guard is dropped still armed, which
        // release() and disarm() both prevent. Needs the runtime to outlive it.
        tokio::spawn(async move {
            if let Err(e) = lock.unlock(job, &token).await {
                tracing::warn!(job, "Could not release run lock (will expire via TTL): {e}");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{acquire, Acquired, RunLock, LOCK_TTL_SECS, MAX_RUN};
    use crate::jobs::test_support::{AcquireOutcome, FakeLock};
    use std::sync::Arc;

    #[test]
    fn lease_outlives_the_run_deadline() {
        assert!(
            LOCK_TTL_SECS > MAX_RUN.as_secs(),
            "the lease must outlast the deadline, or an abandoned run's slot could \
             be taken before its release lands"
        );
    }

    #[tokio::test]
    async fn failed_acquire_releases_before_returning() {
        let fake = FakeLock::new(AcquireOutcome::Fails);
        let lock: Arc<dyn RunLock> = fake.clone();

        let outcome = acquire("job", &lock).await;

        // Asserted with no yield in between: a Drop-spawned unlock could not have
        // run yet, so a nonzero count can only mean the release was awaited inline.
        assert_eq!(
            fake.releases(),
            1,
            "a failed acquire must release inline, not leave it to Drop's spawn"
        );
        assert!(matches!(outcome, Acquired::Failed(_)));
    }

    #[tokio::test]
    async fn held_lock_is_not_released() {
        let fake = FakeLock::new(AcquireOutcome::Denied);
        let lock: Arc<dyn RunLock> = fake.clone();

        let outcome = acquire("job", &lock).await;
        // Yield so a stray Drop-spawned unlock would surface rather than race us.
        tokio::task::yield_now().await;

        assert_eq!(
            fake.releases(),
            0,
            "we never held the lock; unlocking would free another run's lease"
        );
        assert!(matches!(outcome, Acquired::Held));
    }

    #[tokio::test]
    async fn acquire_and_release_use_the_same_job() {
        let fake = FakeLock::new(AcquireOutcome::Granted);
        let lock: Arc<dyn RunLock> = fake.clone();

        let Acquired::Taken(guard) = acquire("the-job", &lock).await else {
            panic!("a granted acquire must yield a guard");
        };
        assert_eq!(fake.releases(), 0, "held until released");
        guard.release().await;

        assert_eq!(fake.releases(), 1);
        assert_eq!(
            fake.acquired_with(),
            fake.released_with(),
            "the release must target the key the acquire took"
        );
    }
}
