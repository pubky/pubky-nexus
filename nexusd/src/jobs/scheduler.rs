use chrono::{DateTime, Utc};
use cron::Schedule;
use opentelemetry::metrics::{Counter, Meter};
use opentelemetry::{global, KeyValue};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch::Receiver;
use tracing::{debug, error, info, warn};

use super::lock::RunLock;
use super::{CronParseError, Job};

/// OpenTelemetry meter name for all job metrics.
const METER_NAME: &str = "nexus.jobs";

/// Shared clock source. Production passes `Arc::new(Utc::now)`; tests pass a
/// virtual clock tied to tokio's paused time.
pub type NowFn = Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>;

/// Drives scheduled jobs, owning the clock, run lock, and metrics. Cheap to
/// clone (all `Arc`-backed), so [`supervise`](super::supervise) hands one clone
/// to each per-job task.
#[derive(Clone)]
pub struct Scheduler {
    now_fn: NowFn,
    lock: Arc<dyn RunLock>,
    /// Every fire attempt, before the lock is tried.
    run_attempts: Counter<u64>,
    /// Fires where the job ran, labelled by `outcome` in {ok, error}.
    run_completed: Counter<u64>,
    /// Fires that didn't run the job, labelled by `reason` in
    /// {in_progress, lock_error}.
    run_skipped: Counter<u64>,
    /// Schedulers that stopped and won't run again until restart, labelled by
    /// `reason` in {panic, schedule_exhausted}. Excludes clean shutdown. A
    /// nonzero rate should alert.
    scheduler_stopped: Counter<u64>,
}

impl Scheduler {
    /// Builds a scheduler with `now_fn` as its clock and `lock` guarding runs.
    /// Instruments come from the global meter (no-ops until an
    /// `SdkMeterProvider` is installed).
    pub fn new(now_fn: NowFn, lock: Arc<dyn RunLock>) -> Self {
        Self::with_meter(now_fn, lock, global::meter(METER_NAME))
    }

    /// [`new`](Self::new) with an explicit `meter`, so tests can install a local
    /// `SdkMeterProvider` and read the counters back.
    pub(crate) fn with_meter(now_fn: NowFn, lock: Arc<dyn RunLock>, meter: Meter) -> Self {
        let run_attempts = meter
            .u64_counter("jobs.run.attempts")
            .with_description("Scheduled fire attempts (before lock acquisition)")
            .build();
        let run_completed = meter
            .u64_counter("jobs.run.completed")
            .with_description("Scheduled fires where the job's run() was invoked")
            .build();
        let run_skipped = meter
            .u64_counter("jobs.run.skipped")
            .with_description("Scheduled fires skipped without running the job")
            .build();
        let scheduler_stopped = meter
            .u64_counter("jobs.scheduler.stopped")
            .with_description(
                "Schedulers that stopped and won't run until restart (excludes shutdown)",
            )
            .build();
        Self {
            now_fn,
            lock,
            run_attempts,
            run_completed,
            run_skipped,
            scheduler_stopped,
        }
    }

    /// Records that a job's scheduler stopped and won't run again until restart
    /// (a caught panic or an exhausted schedule — never a clean shutdown).
    pub(super) fn record_stopped(&self, job: &str, reason: &'static str) {
        self.scheduler_stopped.add(
            1,
            &[
                KeyValue::new("job", job.to_string()),
                KeyValue::new("reason", reason),
            ],
        );
    }
}

/// Waits until `shutdown_rx` carries `true` or all senders drop; `false` sends
/// are ignored.
///
/// Cancel-safe: the only await is `Receiver::changed()`, and there's no await
/// between wake and `borrow_and_update()`, so a `select!` drop can't swallow a
/// shutdown. Keep it that way.
async fn wait_for_shutdown(rx: &mut Receiver<bool>) {
    loop {
        match rx.changed().await {
            Ok(()) => {
                if *rx.borrow_and_update() {
                    return;
                }
                // false — not a shutdown, keep waiting.
            }
            Err(_) => {
                // All senders dropped — treat as shutdown.
                return;
            }
        }
    }
}

/// Parses `cron_expr` and checks it has at least one upcoming fire time,
/// returning the [`Schedule`] or an error. Used to fail fast when building a job.
pub fn validate_cron(cron_expr: &str) -> Result<Schedule, CronParseError> {
    let schedule = Schedule::from_str(cron_expr).map_err(|e| CronParseError {
        expr: cron_expr.into(),
        reason: format!("parse failed: {e}"),
    })?;
    // A valid expression can still never fire (e.g. a fixed past year): the
    // scheduler would start and immediately exit.
    if schedule.upcoming(Utc).next().is_none() {
        return Err(CronParseError {
            expr: cron_expr.into(),
            reason: "has no upcoming fire times".to_string(),
        });
    }
    Ok(schedule)
}

impl Scheduler {
    /// Drives a single [`Job`] on its pre-parsed `schedule` until shutdown. A
    /// failing run is logged but doesn't stop later runs. Each iteration
    /// re-anchors to wall-clock via `schedule.after()`, so a forward clock jump
    /// (NTP, resume) is O(1) with no catch-up per missed tick.
    pub async fn run_job(
        &self,
        name: &str,
        schedule: &Schedule,
        job: &dyn Job,
        mut shutdown_rx: Receiver<bool>,
    ) {
        info!(job = name, cron = %schedule, "Job scheduler started");

        loop {
            // Re-anchor to wall-clock each iteration: O(1) skip of all missed ticks.
            let now = (self.now_fn)();
            let Some(next) = schedule.after(&now).next() else {
                // Bounded schedule (e.g. fixed past year) ran out of fire times.
                warn!(
                    job = name,
                    cron = %schedule,
                    "Cron schedule has no more fire times; scheduled job stopped"
                );
                self.record_stopped(name, "schedule_exhausted");
                break;
            };

            // `after(&now)` returns times strictly after `now`, so the sleep is
            // always positive — no overrun check needed.
            let remaining = (next - now).to_std().unwrap_or(Duration::ZERO);
            tokio::select! {
                biased;

                _ = wait_for_shutdown(&mut shutdown_rx) => {
                    info!(job = name, "Shutdown received, exiting job scheduler");
                    return;
                }
                _ = tokio::time::sleep(remaining) => {}
            }

            debug!(job = name, fire_time = %next, "Running scheduled job");
            tokio::select! {
                biased;

                // Dropping the future leaves the lock to its TTL, but we're exiting anyway.
                _ = wait_for_shutdown(&mut shutdown_rx) => {
                    info!(job = name, "Shutdown received during run; abandoning it");
                    return;
                }
                () = self.run_locked(name, job) => {}
            }
        }
    }

    /// Acquires the run lock, runs the job once, then releases it. Skips (no
    /// error) when the lock is held or unreachable — the next tick retries
    /// rather than piling up a backlog.
    ///
    /// A panic in `run()` propagates via `JoinSet` (see `supervise`); the lock
    /// still releases through `LockGuard`'s Drop.
    async fn run_locked(&self, name: &str, job: &dyn Job) {
        self.run_attempts
            .add(1, &[KeyValue::new("job", name.to_string())]);

        let token = match self.lock.try_lock(name).await {
            Ok(Some(token)) => token,
            Ok(None) => {
                self.run_skipped.add(
                    1,
                    &[
                        KeyValue::new("job", name.to_string()),
                        KeyValue::new("reason", "in_progress"),
                    ],
                );
                warn!(
                    job = name,
                    "Previous run still in progress; skipping this fire"
                );
                return;
            }
            Err(e) => {
                self.run_skipped.add(
                    1,
                    &[
                        KeyValue::new("job", name.to_string()),
                        KeyValue::new("reason", "lock_error"),
                    ],
                );
                error!(
                    job = name,
                    "Could not acquire run lock; skipping this fire: {e}"
                );
                return;
            }
        };

        // `name` isn't &'static; use `job.name()`, which the trait guarantees is.
        let guard = super::lock::LockGuard::new(job.name(), token, self.lock.clone());

        let result = job.run().await;

        // Await the release so unlock errors log synchronously. A panic in run()
        // skips this line — Drop releases instead.
        guard.release().await;

        let outcome = match &result {
            Ok(()) => "ok",
            Err(_) => "error",
        };
        self.run_completed.add(
            1,
            &[
                KeyValue::new("job", name.to_string()),
                KeyValue::new("outcome", outcome),
            ],
        );
        match result {
            Ok(()) => debug!(job = name, "Scheduled job completed"),
            Err(e) => error!(job = name, "Scheduled job failed: {e:?}"),
        }
    }
}

/// A `now_fn` mapping tokio's virtual clock (under `start_paused`) back to
/// wall-clock `DateTime<Utc>`.
#[cfg(test)]
pub(crate) fn virtual_now() -> NowFn {
    let wall0 = Utc::now();
    let virt0 = tokio::time::Instant::now();
    Arc::new(move || {
        wall0
            + chrono::Duration::from_std(virt0.elapsed())
                .expect("virtual elapsed time must fit in chrono::Duration")
    })
}

#[cfg(test)]
mod tests {
    use super::super::lock::{AlwaysAvailableLock, RunLock};
    use super::{validate_cron, virtual_now, NowFn, Scheduler};
    use crate::jobs::Job;
    use async_trait::async_trait;
    use nexus_common::db::RedisResult;
    use opentelemetry::metrics::MeterProvider;
    use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData, ResourceMetrics};
    use opentelemetry_sdk::metrics::{InMemoryMetricExporter, SdkMeterProvider};
    use std::error::Error;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;
    use tokio::sync::watch;

    /// A [`Scheduler`] whose counters feed an in-memory exporter. Call
    /// `provider.force_flush()` before reading `exporter.get_finished_metrics()`.
    fn metered_scheduler(
        now_fn: NowFn,
        lock: Arc<dyn RunLock>,
    ) -> (Scheduler, SdkMeterProvider, InMemoryMetricExporter) {
        let exporter = InMemoryMetricExporter::default();
        let provider = SdkMeterProvider::builder()
            .with_periodic_exporter(exporter.clone())
            .build();
        let scheduler = Scheduler::with_meter(now_fn, lock, provider.meter("test"));
        (scheduler, provider, exporter)
    }

    /// Sum of a `u64` counter's data points whose attributes contain every
    /// (key, value) pair in `filters`.
    fn counter_value(metrics: &[ResourceMetrics], name: &str, filters: &[(&str, &str)]) -> u64 {
        let mut total = 0;
        for rm in metrics {
            for sm in rm.scope_metrics() {
                for m in sm.metrics().filter(|m| m.name() == name) {
                    let AggregatedMetrics::U64(MetricData::Sum(sum)) = m.data() else {
                        continue;
                    };
                    for dp in sum.data_points() {
                        let matches = filters.iter().all(|(k, v)| {
                            dp.attributes().any(|kv| {
                                kv.key.as_str() == *k
                                    && kv.value.as_str() == std::borrow::Cow::Borrowed(*v)
                            })
                        });
                        if matches {
                            total += dp.value();
                        }
                    }
                }
            }
        }
        total
    }

    /// A valid cron expression is accepted.
    #[test]
    fn validate_cron_accepts_valid_expression() {
        assert!(validate_cron("0 0 3 * * *").is_ok());
    }

    /// A malformed cron expression is rejected.
    #[test]
    fn validate_cron_rejects_invalid_expression() {
        assert!(validate_cron("not a cron expression").is_err());
    }

    /// A syntactically valid cron that can never fire (fixed past year) is rejected.
    #[test]
    fn validate_cron_rejects_never_firing() {
        assert!(validate_cron("0 0 3 * * * 2020").is_err());
    }

    /// A run overrunning several fire times gets no back-to-back catch-up runs:
    /// the scheduler skips missed ticks and fires exactly once more.
    #[tokio::test(start_paused = true)]
    async fn overrunning_run_skips_missed_ticks_and_fires_once_more() {
        /// Sleeps past several per-second ticks on its first run only.
        struct OverrunJob {
            fires: Arc<AtomicU32>,
            first_run_sleep: Duration,
        }
        #[async_trait]
        impl Job for OverrunJob {
            fn name(&self) -> &'static str {
                "overrun"
            }
            async fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
                let prior = self.fires.fetch_add(1, Ordering::SeqCst);
                if prior == 0 {
                    tokio::time::sleep(self.first_run_sleep).await;
                }
                Ok(())
            }
        }

        let fires = Arc::new(AtomicU32::new(0));
        let job = OverrunJob {
            fires: fires.clone(),
            first_run_sleep: Duration::from_secs(3),
        };
        let (tx, rx) = watch::channel(false);

        let schedule = validate_cron("* * * * * *").unwrap();
        let scheduler = Scheduler::new(virtual_now(), Arc::new(AlwaysAvailableLock));
        let runner = scheduler.run_job("overrun", &schedule, &job, rx);
        let driver = async {
            // Anchor to the first fire so timing doesn't depend on the start
            // offset within the second.
            while fires.load(Ordering::SeqCst) == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            // Wait past the ~3s run and its single catch-up fire, but before the
            // next tick.
            tokio::time::sleep(Duration::from_millis(4200)).await;
            let _ = tx.send(true);
        };
        tokio::join!(runner, driver);

        assert_eq!(
            fires.load(Ordering::SeqCst),
            2,
            "overrun must skip missed ticks: the first fire plus exactly one catch-up"
        );
    }

    /// Shutdown mid-run abandons the (here never-completing) run and returns
    /// promptly.
    #[tokio::test(start_paused = true)]
    async fn shutdown_during_running_job_returns_promptly() {
        /// Starts, then blocks forever on a `Notify` that is never signaled.
        struct BlockingJob {
            started: Arc<AtomicU32>,
            completed: Arc<AtomicU32>,
            gate: Arc<tokio::sync::Notify>,
        }
        #[async_trait]
        impl Job for BlockingJob {
            fn name(&self) -> &'static str {
                "blocking"
            }
            async fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
                self.started.fetch_add(1, Ordering::SeqCst);
                self.gate.notified().await; // never signaled
                self.completed.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }

        let started = Arc::new(AtomicU32::new(0));
        let completed = Arc::new(AtomicU32::new(0));
        let job = BlockingJob {
            started: started.clone(),
            completed: completed.clone(),
            gate: Arc::new(tokio::sync::Notify::new()),
        };
        let (tx, rx) = watch::channel(false);

        let shutdown_at: Mutex<Option<tokio::time::Instant>> = Mutex::new(None);
        let schedule = validate_cron("* * * * * *").unwrap();
        let scheduler = Scheduler::new(virtual_now(), Arc::new(AlwaysAvailableLock));
        let runner = scheduler.run_job("blocking", &schedule, &job, rx);
        let driver = async {
            // Wait until the job has actually started running.
            while started.load(Ordering::SeqCst) == 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            *shutdown_at.lock().unwrap() = Some(tokio::time::Instant::now());
            let _ = tx.send(true);
        };
        tokio::join!(runner, driver);

        let elapsed = shutdown_at
            .lock()
            .unwrap()
            .expect("the job should have started")
            .elapsed();
        assert_eq!(
            elapsed,
            Duration::ZERO,
            "run_job must return promptly after shutdown mid-run (zero virtual time elapsed); took {elapsed:?}"
        );
        assert_eq!(
            started.load(Ordering::SeqCst),
            1,
            "the job must have started before shutdown"
        );
        assert_eq!(
            completed.load(Ordering::SeqCst),
            0,
            "the abandoned run must not have completed"
        );
    }

    /// A minimal job that just counts how many times it ran.
    struct CountingJob {
        count: Arc<AtomicU32>,
    }
    #[async_trait]
    impl Job for CountingJob {
        fn name(&self) -> &'static str {
            "counting"
        }
        async fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    /// Shutdown during the sleep phase (first select!) exits without firing. Uses
    /// a far-future cron so the job never fires on its own.
    #[tokio::test(start_paused = true)]
    async fn run_job_exits_on_shutdown_without_firing() {
        let count = Arc::new(AtomicU32::new(0));
        let job = CountingJob {
            count: count.clone(),
        };
        let (tx, rx) = watch::channel(false);

        // Year-pinned cron (2100, the crate's max) fixes one fire far outside any
        // test run. The pin is what guarantees no fire: an unpinned "0 0 0 1 1 *"
        // recurs yearly and could fire if the paused clock lands near Jan 1 UTC.
        let schedule = validate_cron("0 0 0 1 1 * 2100").unwrap();
        let scheduler = Scheduler::new(virtual_now(), Arc::new(AlwaysAvailableLock));
        let runner = scheduler.run_job("counting", &schedule, &job, rx);
        let stopper = async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let _ = tx.send(true);
        };
        tokio::join!(runner, stopper);

        assert_eq!(
            count.load(Ordering::SeqCst),
            0,
            "shutdown during sleep must exit without firing the job"
        );
    }

    /// A `false` send must not stop the scheduler; it exits only on `true` (or
    /// dropped senders).
    #[tokio::test(start_paused = true)]
    async fn false_shutdown_signal_does_not_stop_scheduler() {
        let count = Arc::new(AtomicU32::new(0));
        let job = CountingJob {
            count: count.clone(),
        };
        let (tx, rx) = watch::channel(false);

        let schedule = validate_cron("* * * * * *").unwrap();
        let scheduler = Scheduler::new(virtual_now(), Arc::new(AlwaysAvailableLock));
        let runner = scheduler.run_job("counting", &schedule, &job, rx);
        let driver = async {
            // Wait for the first fire, then send false (a no-op).
            while count.load(Ordering::SeqCst) == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            let _ = tx.send(false);
            // Let the scheduler run for a couple more ticks.
            tokio::time::sleep(Duration::from_millis(2500)).await;
            let _ = tx.send(true);
        };
        tokio::join!(runner, driver);

        assert!(
            count.load(Ordering::SeqCst) >= 2,
            "a false send must not stop the scheduler; it should keep firing"
        );
    }

    /// A lock held elsewhere (`try_lock` → `None`) makes the scheduler skip every
    /// fire — the job never runs — while it keeps ticking and exits on shutdown.
    #[tokio::test(start_paused = true)]
    async fn run_job_skips_run_when_lock_held() {
        struct HeldLock;
        #[async_trait]
        impl RunLock for HeldLock {
            async fn try_lock(&self, _job: &str) -> RedisResult<Option<String>> {
                Ok(None)
            }
            async fn unlock(&self, _job: &str, _token: &str) -> RedisResult<()> {
                Ok(())
            }
        }

        let count = Arc::new(AtomicU32::new(0));
        let job = CountingJob {
            count: count.clone(),
        };
        let (tx, rx) = watch::channel(false);

        let schedule = validate_cron("* * * * * *").unwrap();
        let scheduler = Scheduler::new(virtual_now(), Arc::new(HeldLock));
        let runner = scheduler.run_job("counting", &schedule, &job, rx);
        let stopper = async {
            // Several ticks pass; every fire should be skipped.
            tokio::time::sleep(Duration::from_millis(3500)).await;
            let _ = tx.send(true);
        };
        tokio::join!(runner, stopper);

        assert_eq!(
            count.load(Ordering::SeqCst),
            0,
            "a held lock must prevent the job from running"
        );
    }

    /// Every fire pairs one lock acquire with one release, so the lock is never
    /// leaked between runs.
    #[tokio::test(start_paused = true)]
    async fn run_job_acquires_and_releases_lock_around_run() {
        struct RecordingLock {
            acquired: Arc<AtomicU32>,
            released: Arc<AtomicU32>,
        }
        #[async_trait]
        impl RunLock for RecordingLock {
            async fn try_lock(&self, _job: &str) -> RedisResult<Option<String>> {
                self.acquired.fetch_add(1, Ordering::SeqCst);
                Ok(Some("token".to_string()))
            }
            async fn unlock(&self, _job: &str, _token: &str) -> RedisResult<()> {
                self.released.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }

        let acquired = Arc::new(AtomicU32::new(0));
        let released = Arc::new(AtomicU32::new(0));
        let count = Arc::new(AtomicU32::new(0));
        let job = CountingJob {
            count: count.clone(),
        };
        let (tx, rx) = watch::channel(false);

        let schedule = validate_cron("* * * * * *").unwrap();
        let lock = RecordingLock {
            acquired: acquired.clone(),
            released: released.clone(),
        };
        let scheduler = Scheduler::new(virtual_now(), Arc::new(lock));
        let runner = scheduler.run_job("counting", &schedule, &job, rx);
        let stopper = async {
            tokio::time::sleep(Duration::from_millis(2500)).await;
            let _ = tx.send(true);
        };
        tokio::join!(runner, stopper);

        let runs = count.load(Ordering::SeqCst);
        assert!(runs >= 2, "the job should have fired at least twice");
        assert_eq!(
            acquired.load(Ordering::SeqCst),
            released.load(Ordering::SeqCst),
            "every acquire must be paired with a release"
        );
        assert_eq!(
            released.load(Ordering::SeqCst),
            runs,
            "the lock must be released once per run"
        );
    }

    /// Every fire is one attempt that partitions into completed or skipped,
    /// never both — asserted by reading the real `jobs.run.*` OTel counters back
    /// through an in-memory exporter.
    #[tokio::test(start_paused = true)]
    async fn attempts_partition_between_completed_and_skipped() {
        /// Grants then denies the lock on alternate fires: even fires run the job
        /// (completed{ok}), odd fires are denied (skipped{in_progress}).
        struct AlternatingLock {
            calls: AtomicU32,
        }
        #[async_trait]
        impl RunLock for AlternatingLock {
            async fn try_lock(&self, _job: &str) -> RedisResult<Option<String>> {
                if self.calls.fetch_add(1, Ordering::SeqCst).is_multiple_of(2) {
                    Ok(Some("token".to_string()))
                } else {
                    Ok(None)
                }
            }
            async fn unlock(&self, _job: &str, _token: &str) -> RedisResult<()> {
                Ok(())
            }
        }

        let count = Arc::new(AtomicU32::new(0));
        let job = CountingJob {
            count: count.clone(),
        };
        let (tx, rx) = watch::channel(false);

        let schedule = validate_cron("* * * * * *").unwrap();
        let (scheduler, provider, exporter) = metered_scheduler(
            virtual_now(),
            Arc::new(AlternatingLock {
                calls: AtomicU32::new(0),
            }),
        );
        let runner = scheduler.run_job("counting", &schedule, &job, rx);
        let stopper = async {
            tokio::time::sleep(Duration::from_millis(4500)).await;
            let _ = tx.send(true);
        };
        tokio::join!(runner, stopper);

        provider.force_flush().unwrap();
        let metrics = exporter.get_finished_metrics().unwrap();

        let attempts = counter_value(&metrics, "jobs.run.attempts", &[("job", "counting")]);
        let completed = counter_value(
            &metrics,
            "jobs.run.completed",
            &[("job", "counting"), ("outcome", "ok")],
        );
        let skipped = counter_value(
            &metrics,
            "jobs.run.skipped",
            &[("job", "counting"), ("reason", "in_progress")],
        );

        assert!(attempts >= 3, "the alternating lock needs several fires");
        assert!(
            completed >= 1 && skipped >= 1,
            "both the completed and skipped branches must be exercised (completed={completed}, skipped={skipped})"
        );
        assert_eq!(
            attempts,
            completed + skipped,
            "every fire attempt must partition into exactly one of completed or skipped"
        );
    }

    /// A bounded schedule that runs out of fire times records one
    /// `jobs.scheduler.stopped{reason=schedule_exhausted}` and returns on its own.
    #[tokio::test(start_paused = true)]
    async fn exhausted_schedule_records_stopped_metric() {
        let count = Arc::new(AtomicU32::new(0));
        let job = CountingJob {
            count: count.clone(),
        };
        // No shutdown is sent: run_job returns once the schedule is exhausted.
        let (_tx, rx) = watch::channel(false);

        // Year-pinned single fire: fires once, then `after()` yields no more times.
        let schedule = validate_cron("0 0 0 1 1 * 2099").unwrap();
        let (scheduler, provider, exporter) =
            metered_scheduler(virtual_now(), Arc::new(AlwaysAvailableLock));
        scheduler.run_job("counting", &schedule, &job, rx).await;

        provider.force_flush().unwrap();
        let metrics = exporter.get_finished_metrics().unwrap();

        assert!(
            count.load(Ordering::SeqCst) >= 1,
            "the single-fire schedule must fire before exhausting"
        );
        assert_eq!(
            counter_value(
                &metrics,
                "jobs.scheduler.stopped",
                &[("job", "counting"), ("reason", "schedule_exhausted")],
            ),
            1,
            "an exhausted schedule must record exactly one stopped{{schedule_exhausted}}"
        );
    }
}
