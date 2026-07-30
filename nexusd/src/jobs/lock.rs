use async_trait::async_trait;
use nexus_common::db::{release_lock, try_acquire_lock, RedisError};
use opentelemetry::metrics::{Counter, Histogram, Meter};
use opentelemetry::{global, KeyValue};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::time::timeout;

use super::error::{LockError, LockResult};
use super::METER_NAME;

/// The one knob: how long a run may take. Callers abandon `run()` at this
/// wall-clock deadline, and the lease is sized from it, so a run can't outlive
/// its lease and overlap another process's.
pub(super) const MAX_RUN: Duration = Duration::from_secs(3600);

/// Lease slack past [`MAX_RUN`], covering the acquire-to-run gap and the release
/// round-trip. Also the crash backstop: after a hard kill the slot frees itself
/// this long after the deadline.
const LEASE_MARGIN: Duration = Duration::from_secs(60);

/// Deadline for a single lock round trip. A backend isn't required to bound its
/// own calls, so without this a stalled one hangs the run phase and, worse,
/// shutdown.
/// Monotonic on purpose: `tokio::time::timeout`, not `sleep_wall`. The
/// wall-clock discipline elsewhere exists so a run can't outlive its lease;
/// a five-second I/O window is unaffected by host suspend.
const LOCK_IO_TIMEOUT: Duration = Duration::from_secs(5);

/// Lease TTL, sized from the deadline it has to outlast.
pub const LOCK_TTL_SECS: u64 = MAX_RUN.as_secs() + LEASE_MARGIN.as_secs();

/// Cross-process mutual exclusion for a job's runs (the scheduler already
/// serializes within one process). Injected so the scheduler is testable
/// without a real backend.
///
/// A claim must expire on its own, so a crashed holder's slot frees itself; that
/// lease has to outlast [`MAX_RUN`].
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
    /// Backend error while acquiring. Inline release was attempted; `released` is
    /// true when that unlock succeeded.
    Failed { error: LockError, released: bool },
    /// I/O deadline elapsed while acquiring. Inline release was attempted;
    /// `released` is true when that unlock succeeded, false when it also timed out.
    TimedOut { released: bool },
}

/// Outcome of a [`LockGuard::release`] operation.
#[derive(Debug)]
pub(super) enum ReleaseOutcome {
    /// Lock was successfully released.
    Released,
    /// Guard was already disarmed; nothing to release.
    NotHeld,
    /// Backend reported an error releasing the lock.
    Failed,
    /// I/O deadline elapsed while releasing. The slot will now stay held until
    /// its lease expires.
    TimedOut,
}

/// Lock-level observability: counter for acquire/release outcomes and a
/// duration histogram for both operations. Cheap to clone (all `Arc`-backed).
/// Instruments are from the global meter (no-ops until an `SdkMeterProvider`
/// is installed). Note: on the scheduled path, lock outcomes overlap
/// `jobs.run.skipped{reason}` — do not sum both on dashboards.
#[derive(Clone)]
pub struct LockMetrics {
    operations_counter: Counter<u64>,
    duration_histogram: Histogram<f64>,
}

impl LockMetrics {
    /// Build from the global meter.
    pub fn new() -> Self {
        Self::with_meter(global::meter(METER_NAME))
    }

    /// Explicit meter for tests.
    pub(crate) fn with_meter(meter: Meter) -> Self {
        let operations_counter = meter
            .u64_counter("jobs.lock.operations")
            .with_description("Lock acquire/release attempts")
            .build();
        let duration_histogram = meter
            .f64_histogram("jobs.lock.duration")
            .with_description("Lock acquire/release duration, in milliseconds")
            .with_unit("ms")
            .build();
        Self {
            operations_counter,
            duration_histogram,
        }
    }

    fn record(
        &self,
        job: &'static str,
        operation: &'static str,
        outcome: &'static str,
        path: &'static str,
        duration: Duration,
    ) {
        let tags = [
            KeyValue::new("job", job),
            KeyValue::new("operation", operation),
            KeyValue::new("outcome", outcome),
            KeyValue::new("path", path),
        ];
        self.operations_counter.add(1, &tags);
        self.duration_histogram
            .record(duration.as_secs_f64() * 1000.0, &tags);
    }

    fn record_acquire(&self, job: &'static str, outcome: &'static str, duration: Duration) {
        self.record(job, "acquire", outcome, "inline", duration);
    }

    fn record_release(&self, job: &'static str, outcome: &'static str, duration: Duration) {
        self.record(job, "release", outcome, "inline", duration);
    }

    fn record_release_from_drop(
        &self,
        job: &'static str,
        outcome: &'static str,
        duration: Duration,
    ) {
        self.record(job, "release", outcome, "drop", duration);
    }
}

/// Takes `job`'s run slot, arming the guard *before* the acquire so a lost
/// acquire reply (or a cancel mid-acquire) still releases; the release is
/// token-scoped, so it no-ops if our token never took the slot.
///
/// One `job` feeds both the acquire and the guard, so a run can't release a key
/// it never acquired. On backend error the guard is released inline rather than
/// left to Drop: callers may return straight into process exit, where a
/// Drop-spawned unlock never gets polled.
// Drop-spawned release may cancel at shutdown — TTL is the backstop.
// Acquire-timeout ownership uncertain — best-effort inline release, TTL backstop.
pub(super) async fn acquire(
    job: &'static str,
    lock: &Arc<dyn RunLock>,
    metrics: &LockMetrics,
) -> Acquired {
    let token = lock.new_token();
    let guard = LockGuard::new(job, token.clone(), Arc::clone(lock), metrics.clone());

    let start = Instant::now();
    match timeout(LOCK_IO_TIMEOUT, lock.acquire(job, &token)).await {
        Ok(Ok(true)) => {
            metrics.record_acquire(job, "ok", start.elapsed());
            Acquired::Taken(guard)
        }
        Ok(Ok(false)) => {
            metrics.record_acquire(job, "held", start.elapsed());
            guard.disarm();
            Acquired::Held
        }
        Ok(Err(e)) => {
            let elapsed = start.elapsed();
            let released = matches!(guard.release().await, ReleaseOutcome::Released);
            metrics.record_acquire(job, "error", elapsed);
            Acquired::Failed { error: e, released }
        }
        Err(_) => {
            let elapsed = start.elapsed();
            let released = matches!(guard.release().await, ReleaseOutcome::Released);
            metrics.record_acquire(job, "timeout", elapsed);
            Acquired::TimedOut { released }
        }
    }
}

/// RAII lock release. Prefer `release()`, which awaits the unlock; Drop is the
/// backstop for guards dropped while still armed (a panic in `run()`, or the
/// whole future being dropped).
///
/// Drop releases fire-and-forget via `tokio::spawn`, so it must be dropped in a
/// runtime context, and only frees the lock if that runtime outlives the spawn —
/// otherwise the lease expiry is the fallback. Don't rely on it where the
/// process may exit right after (see [`acquire`]).
pub struct LockGuard {
    job: &'static str,
    token: Option<String>,
    lock: Arc<dyn RunLock>,
    metrics: LockMetrics,
}

impl LockGuard {
    pub(super) fn new(
        job: &'static str,
        token: String,
        lock: Arc<dyn RunLock>,
        metrics: LockMetrics,
    ) -> Self {
        Self {
            job,
            token: Some(token),
            lock,
            metrics,
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
    pub async fn release(mut self) -> ReleaseOutcome {
        let Some(token) = self.token.take() else {
            return ReleaseOutcome::NotHeld;
        };

        let start = Instant::now();
        match timeout(LOCK_IO_TIMEOUT, self.lock.unlock(self.job, &token)).await {
            Ok(Ok(())) => {
                self.metrics.record_release(self.job, "ok", start.elapsed());
                ReleaseOutcome::Released
            }
            Ok(Err(e)) => {
                self.metrics
                    .record_release(self.job, "error", start.elapsed());
                tracing::debug!(job = self.job, "Could not release run lock: {e}");
                ReleaseOutcome::Failed
            }
            Err(_) => {
                self.metrics
                    .record_release(self.job, "timeout", start.elapsed());
                tracing::debug!(
                    job = self.job,
                    "Unlock timed out — slot may stay held until TTL expires"
                );
                ReleaseOutcome::TimedOut
            }
        }
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let Some(token) = self.token.take() else {
            return;
        };
        let lock = self.lock.clone();
        let metrics = self.metrics.clone();
        let job = self.job;
        // Fire-and-forget: reached whenever a guard is dropped still armed, which
        // release() and disarm() both prevent. Needs the runtime to outlive it.
        tokio::spawn(async move {
            let start = Instant::now();
            match timeout(LOCK_IO_TIMEOUT, lock.unlock(job, &token)).await {
                Ok(Ok(())) => {
                    metrics.record_release_from_drop(job, "ok", start.elapsed());
                }
                Ok(Err(e)) => {
                    metrics.record_release_from_drop(job, "error", start.elapsed());
                    // Sole reporter (no caller), so stays warn unlike release()
                    tracing::warn!(job, "Could not release run lock: {e}");
                }
                Err(_) => {
                    metrics.record_release_from_drop(job, "timeout", start.elapsed());
                    // Sole reporter (no caller), so stays warn unlike release()
                    tracing::warn!(
                        job,
                        "Drop-path unlock timed out — slot may stay held until TTL expires"
                    );
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{acquire, LockMetrics, ReleaseOutcome, LOCK_IO_TIMEOUT};
    use super::{Acquired, RunLock, LEASE_MARGIN, LOCK_TTL_SECS, MAX_RUN};
    use crate::jobs::test_support::{counter_value, AcquireOutcome, FakeLock, UnlockOutcome};
    use opentelemetry::metrics::MeterProvider;
    use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData};
    use opentelemetry_sdk::metrics::{InMemoryMetricExporter, SdkMeterProvider};
    use std::sync::Arc;

    /// Build a [`LockMetrics`] with an in-memory exporter.
    fn metered_lock_metrics() -> (LockMetrics, SdkMeterProvider, InMemoryMetricExporter) {
        let exporter = InMemoryMetricExporter::default();
        let provider = SdkMeterProvider::builder()
            .with_periodic_exporter(exporter.clone())
            .build();
        let metrics = LockMetrics::with_meter(provider.meter("test"));
        (metrics, provider, exporter)
    }

    #[test]
    fn lock_io_timeout_is_comfortably_within_lease_margin() {
        // The margin covers the acquire-to-run gap plus the release round trip.
        // 2 * LOCK_IO_TIMEOUT is harmless slack for both I/O operations.
        assert!(
            2 * LOCK_IO_TIMEOUT < LEASE_MARGIN,
            "2 * LOCK_IO_TIMEOUT ({:?}) must be < LEASE_MARGIN ({:?}) so that the \
             acquire-to-run gap + release round trip fit within the margin",
            2 * LOCK_IO_TIMEOUT,
            LEASE_MARGIN,
        );
    }

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
        let fake = FakeLock::new(AcquireOutcome::Fails, UnlockOutcome::Succeeds);
        let lock: Arc<dyn RunLock> = fake.clone();
        let (metrics, _provider, _exporter) = metered_lock_metrics();

        let outcome = acquire("job", &lock, &metrics).await;

        // Asserted with no yield in between: a Drop-spawned unlock could not have
        // run yet, so a nonzero count can only mean the release was awaited inline.
        assert_eq!(
            fake.unlock_attempts(),
            1,
            "a failed acquire must release inline, not leave it to Drop's spawn"
        );
        let Acquired::Failed { error, released } = outcome else {
            panic!("expected Acquired::Failed");
        };
        assert!(released, "inline release should succeed when unlock works");
        assert!(
            error.to_string().contains("backend down"),
            "the original backend error should be preserved"
        );
    }

    #[tokio::test]
    async fn held_lock_is_not_released() {
        let fake = FakeLock::new(AcquireOutcome::Denied, UnlockOutcome::Succeeds);
        let lock: Arc<dyn RunLock> = fake.clone();
        let (metrics, _provider, _exporter) = metered_lock_metrics();

        let outcome = acquire("job", &lock, &metrics).await;
        // Yield so a stray Drop-spawned unlock would surface rather than race us.
        tokio::task::yield_now().await;

        assert_eq!(
            fake.unlock_attempts(),
            0,
            "we never held the lock; unlocking would free another run's lease"
        );
        assert!(matches!(outcome, Acquired::Held));
    }

    #[tokio::test]
    async fn acquire_and_release_use_the_same_job() {
        let fake = FakeLock::new(AcquireOutcome::Granted, UnlockOutcome::Succeeds);
        let lock: Arc<dyn RunLock> = fake.clone();
        let (metrics, _provider, _exporter) = metered_lock_metrics();

        let Acquired::Taken(guard) = acquire("the-job", &lock, &metrics).await else {
            panic!("a granted acquire must yield a guard");
        };
        assert_eq!(fake.unlock_attempts(), 0, "held until released");
        guard.release().await;

        assert_eq!(fake.unlock_attempts(), 1);
        assert_eq!(
            fake.acquired_with(),
            fake.released_with(),
            "the release must target the key the acquire took"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn acquire_times_out_and_releases_inline() {
        let fake = FakeLock::new(AcquireOutcome::Hangs, UnlockOutcome::Succeeds);
        let lock: Arc<dyn RunLock> = fake.clone();
        let (metrics, _provider, _exporter) = metered_lock_metrics();

        // Paused-mode auto-advance fires the timer when the runtime goes idle on pending().
        let outcome = acquire("job", &lock, &metrics).await;

        let Acquired::TimedOut { released } = outcome else {
            panic!("expected Acquired::TimedOut");
        };
        assert!(released, "inline release should succeed when unlock works");
        assert_eq!(
            fake.unlock_attempts(),
            1,
            "the timeout path must release inline, not leave it to Drop"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn acquire_and_release_both_time_out() {
        let fake = FakeLock::new(AcquireOutcome::Hangs, UnlockOutcome::Hangs);
        let lock: Arc<dyn RunLock> = fake.clone();
        let (metrics, _provider, _exporter) = metered_lock_metrics();

        let outcome = acquire("job", &lock, &metrics).await;

        let Acquired::TimedOut { released } = outcome else {
            panic!("expected Acquired::TimedOut");
        };
        assert!(
            !released,
            "inline release must time out when unlock hangs, so released is false"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn failed_acquire_with_failed_release() {
        let fake = FakeLock::new(AcquireOutcome::Fails, UnlockOutcome::Hangs);
        let lock: Arc<dyn RunLock> = fake.clone();
        let (metrics, _provider, _exporter) = metered_lock_metrics();

        let outcome = acquire("job", &lock, &metrics).await;

        let Acquired::Failed { error, released } = outcome else {
            panic!("expected Acquired::Failed");
        };
        assert!(
            !released,
            "inline release must time out when unlock hangs, so released is false"
        );
        assert!(
            error.to_string().contains("backend down"),
            "the original backend error should be preserved"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn release_times_out_and_reports_it() {
        let fake = FakeLock::new(AcquireOutcome::Granted, UnlockOutcome::Hangs);
        let lock: Arc<dyn RunLock> = fake.clone();
        let (metrics, _provider, _exporter) = metered_lock_metrics();

        let Acquired::Taken(guard) = acquire("job", &lock, &metrics).await else {
            panic!("acquire should succeed");
        };

        // Paused-mode auto-advance fires the timer when the runtime goes idle on pending().
        let outcome = guard.release().await;

        assert!(
            matches!(outcome, ReleaseOutcome::TimedOut),
            "release should return TimedOut when the unlock I/O deadline elapses"
        );
    }

    #[tokio::test]
    async fn lock_metrics_record_acquire_outcomes() {
        let fake = FakeLock::new(AcquireOutcome::Granted, UnlockOutcome::Succeeds);
        let lock: Arc<dyn RunLock> = fake.clone();
        let (metrics, provider, exporter) = metered_lock_metrics();

        let outcome = acquire("test-job", &lock, &metrics).await;
        assert!(matches!(outcome, Acquired::Taken(_)));

        provider.force_flush().unwrap();
        let resource_metrics = exporter.get_finished_metrics().unwrap();

        assert_eq!(
            counter_value(
                &resource_metrics,
                "jobs.lock.operations",
                &[("outcome", "ok"), ("operation", "acquire")]
            ),
            1,
            "acquire outcome must be recorded"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn lock_metrics_record_acquire_timeout() {
        let fake = FakeLock::new(AcquireOutcome::Hangs, UnlockOutcome::Succeeds);
        let lock: Arc<dyn RunLock> = fake.clone();
        let (metrics, provider, exporter) = metered_lock_metrics();

        let outcome = acquire("test-job", &lock, &metrics).await;
        assert!(matches!(outcome, Acquired::TimedOut { .. }));

        provider.force_flush().unwrap();
        let resource_metrics = exporter.get_finished_metrics().unwrap();

        assert_eq!(
            counter_value(
                &resource_metrics,
                "jobs.lock.operations",
                &[("outcome", "timeout"), ("operation", "acquire")]
            ),
            1,
            "acquire timeout must be recorded"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn lock_metrics_record_acquire_error() {
        let fake = FakeLock::new(AcquireOutcome::Fails, UnlockOutcome::Succeeds);
        let lock: Arc<dyn RunLock> = fake.clone();
        let (metrics, provider, exporter) = metered_lock_metrics();

        let outcome = acquire("test-job", &lock, &metrics).await;
        assert!(matches!(outcome, Acquired::Failed { .. }));

        provider.force_flush().unwrap();
        let resource_metrics = exporter.get_finished_metrics().unwrap();

        assert_eq!(
            counter_value(
                &resource_metrics,
                "jobs.lock.operations",
                &[("outcome", "error"), ("operation", "acquire")]
            ),
            1,
            "acquire error must be recorded"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn lock_metrics_record_release_outcomes() {
        let fake = FakeLock::new(AcquireOutcome::Granted, UnlockOutcome::Succeeds);
        let lock: Arc<dyn RunLock> = fake.clone();
        let (metrics, provider, exporter) = metered_lock_metrics();

        let Acquired::Taken(guard) = acquire("test-job", &lock, &metrics).await else {
            panic!("acquire should succeed");
        };
        guard.release().await;

        provider.force_flush().unwrap();
        let resource_metrics = exporter.get_finished_metrics().unwrap();

        assert_eq!(
            counter_value(
                &resource_metrics,
                "jobs.lock.operations",
                &[("outcome", "ok"), ("operation", "release")]
            ),
            1,
            "release outcome must be recorded"
        );
        // Histogram has two series: one acquire + one release (each a unique attr set)
        let mut histogram_count = 0u64;
        for rm in &resource_metrics {
            for sm in rm.scope_metrics() {
                for m in sm.metrics().filter(|m| m.name() == "jobs.lock.duration") {
                    let AggregatedMetrics::F64(MetricData::Histogram(h)) = m.data() else {
                        continue;
                    };
                    histogram_count += h.data_points().count() as u64;
                }
            }
        }
        assert!(
            histogram_count >= 2,
            "duration histogram must have at least 2 series (acquire + release), got {histogram_count}"
        );
    }

    #[tokio::test]
    async fn lock_metrics_record_drop_path_release() {
        let fake = FakeLock::new(AcquireOutcome::Granted, UnlockOutcome::Succeeds);
        let lock: Arc<dyn RunLock> = fake.clone();
        let (metrics, provider, exporter) = metered_lock_metrics();

        let Acquired::Taken(guard) = acquire("test-job", &lock, &metrics).await else {
            panic!("acquire should succeed");
        };
        // Drop the guard without calling release() — triggers the drop path.
        std::mem::drop(guard);
        // Yield so the spawned drop-task runs.
        tokio::task::yield_now().await;

        provider.force_flush().unwrap();
        let resource_metrics = exporter.get_finished_metrics().unwrap();

        assert_eq!(
            counter_value(
                &resource_metrics,
                "jobs.lock.operations",
                &[
                    ("outcome", "ok"),
                    ("operation", "release"),
                    ("path", "drop")
                ]
            ),
            1,
            "drop-path release must be recorded with path=drop"
        );
    }
}
