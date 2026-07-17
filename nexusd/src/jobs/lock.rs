use async_trait::async_trait;
use nexus_common::db::{release_lock, try_acquire_lock, RedisError};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::error::{LockError, LockResult};

/// TTL on a job's run lock, and the crash backstop. Must exceed the longest
/// expected run, or a run outliving its lease could overlap another.
const LOCK_TTL_SECS: u64 = 3600;

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

/// RAII lock release. Call `release()` on the happy path to await unlock; on a
/// panic the explicit call is skipped but Drop still frees the lock so it
/// doesn't stay stuck for its TTL.
///
/// Drop releases fire-and-forget via `tokio::spawn` (so it must be dropped
/// inside a runtime context), falling back to the TTL if the spawn is dropped
/// during shutdown.
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
        // Fire-and-forget. Only reached on the panic path — happy path calls
        // release() first, which sets self.token = None.
        tokio::spawn(async move {
            if let Err(e) = lock.unlock(job, &token).await {
                tracing::warn!(
                    job,
                    "Could not release run lock on panic path (will expire via TTL): {e}"
                );
            }
        });
    }
}

/// Test [`RunLock`] that always grants the lock.
#[cfg(test)]
pub struct AlwaysAvailableLock;

#[cfg(test)]
#[async_trait]
impl RunLock for AlwaysAvailableLock {
    fn new_token(&self) -> String {
        "test-token".to_string()
    }
    async fn acquire(&self, _job: &str, _token: &str) -> LockResult<bool> {
        Ok(true)
    }
    async fn unlock(&self, _job: &str, _token: &str) -> LockResult<()> {
        Ok(())
    }
}
