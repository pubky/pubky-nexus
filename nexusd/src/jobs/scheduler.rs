use chrono::{DateTime, Utc};
use cron::Schedule;
use opentelemetry::metrics::{Counter, Meter};
use opentelemetry::{global, KeyValue};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch::Receiver;
use tracing::{debug, error, info, warn};

use super::lock::{self, RunLock};
use super::{CronParseError, Job};

/// OpenTelemetry meter name for all job metrics.
const METER_NAME: &str = "nexus.jobs";

/// Wall-clock re-check interval while waiting for a fire time (see [`Scheduler::sleep_until`]).
pub(super) const MAX_SLEEP: Duration = Duration::from_secs(30);

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
    /// Wall-clock re-check interval while sleeping toward a fire time (see [`MAX_SLEEP`]).
    max_sleep: Duration,
    /// Deadline for a single run, past which it's abandoned (see [`lock::MAX_RUN`]).
    max_run: Duration,
    /// Every fire attempt, before the lock is tried.
    run_attempts: Counter<u64>,
    /// Fires where the job ran, labelled by `outcome` in {ok, error, abandoned, timed_out}.
    run_completed: Counter<u64>,
    /// Fires that didn't run the job, labelled by `reason` in
    /// {in_progress, lock_error, shutdown_before_acquire}.
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
            max_sleep: MAX_SLEEP,
            max_run: lock::MAX_RUN,
            run_attempts,
            run_completed,
            run_skipped,
            scheduler_stopped,
        }
    }

    /// Overrides the wall-clock re-check interval. Only for tests pinning a
    /// far-future fire time, where the default would grind through millions of
    /// chunks under a paused clock.
    #[cfg(test)]
    pub(crate) fn with_max_sleep(mut self, max_sleep: Duration) -> Self {
        self.max_sleep = max_sleep;
        self
    }

    /// Overrides the per-run deadline, so tests can hit it without burning the
    /// production hour of virtual time.
    #[cfg(test)]
    pub(crate) fn with_max_run(mut self, max_run: Duration) -> Self {
        self.max_run = max_run;
        self
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

/// Sleeps for `dur` of *wall-clock* time, re-reading `now_fn` after each chunk of
/// at most `max_sleep`.
///
/// Same reason as [`Scheduler::sleep_until`]: `tokio::time::sleep` counts
/// monotonic time, which stalls across a host suspend. The run lock's lease is a
/// wall-clock TTL, so a monotonic deadline would let a suspend push a run past
/// its expired lease and alongside another process's run.
pub(super) async fn sleep_wall(dur: Duration, now_fn: &NowFn, max_sleep: Duration) {
    let started = now_fn();
    loop {
        // A backward clock step reads as no elapsed time rather than as overshoot.
        let elapsed = (now_fn() - started).to_std().unwrap_or(Duration::ZERO);
        let Some(remaining) = dur.checked_sub(elapsed) else {
            return;
        };
        if remaining.is_zero() {
            return;
        }
        tokio::time::sleep(remaining.min(max_sleep)).await;
    }
}

/// Waits until `shutdown_rx` carries `true` or all senders drop; `false` sends
/// are ignored. Edge-triggered: an already-seen `true` won't wake it.
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
    /// failing run is logged but doesn't stop later runs. Wall-clock jumps in
    /// either direction are handled at the anchor and in [`sleep_until`](Self::sleep_until).
    pub async fn run_job(
        &self,
        schedule: &Schedule,
        job: &dyn Job,
        mut shutdown_rx: Receiver<bool>,
    ) {
        // Single source of truth for the name: the lock key and the metric labels
        // must agree, or a run releases a key it never acquired.
        let name = job.name();
        info!(job = name, cron = %schedule, "Job scheduler started");

        // The last tick fired, so a backward clock step can't hand it back.
        let mut last_fired: Option<DateTime<Utc>> = None;

        loop {
            // `wait_for_shutdown` only fires on the false→true transition, and
            // run_locked may already have consumed it while abandoning a run. Read
            // the current value directly so an already-signaled shutdown still exits.
            if *shutdown_rx.borrow() {
                info!(job = name, "Shutdown received, exiting job scheduler");
                return;
            }

            // Re-anchor to wall-clock each iteration: O(1) skip of all missed ticks.
            // Anchor past `last_fired` too — a backward step (NTP, snapshot restore)
            // leaves `now` before a tick we already ran, and `after()` would return
            // that same tick for a second run.
            let now = (self.now_fn)();
            let anchor = last_fired.map_or(now, |last| last.max(now));
            let Some(next) = schedule.after(&anchor).next() else {
                // Bounded schedule (e.g. fixed past year) ran out of fire times.
                warn!(
                    job = name,
                    cron = %schedule,
                    "Cron schedule has no more fire times; scheduled job stopped"
                );
                self.record_stopped(name, "schedule_exhausted");
                break;
            };

            // Sleep phase: shutdown can drop this safely — no lock I/O here.
            if !self.sleep_until(next, &mut shutdown_rx).await {
                info!(job = name, "Shutdown received, exiting job scheduler");
                return;
            }

            // Consumed whether or not the run below is skipped: this tick is spent
            // either way, and retrying it would double-run the job.
            last_fired = Some(next);

            debug!(job = name, fire_time = %next, "Running scheduled job");
            // Run phase: no select! around run_locked — that would drop it (and its
            // in-flight lock I/O) on shutdown. run_locked observes shutdown itself,
            // as a signal that leaves acquire and release uncancellable.
            self.run_locked(job, &mut shutdown_rx).await;
        }
    }

    /// Sleeps until the wall clock reaches `deadline`, re-reading it after each
    /// chunk of at most `max_sleep`. Returns `false` if shutdown was signaled
    /// instead.
    ///
    /// Chunking is what makes the deadline wall-clock-based: `tokio::time::sleep`
    /// counts monotonic time, which doesn't advance across a host suspend, so one
    /// `sleep(deadline - now)` would fire late by the suspend's full duration.
    async fn sleep_until(&self, deadline: DateTime<Utc>, shutdown: &mut Receiver<bool>) -> bool {
        loop {
            // Negative (deadline already passed) converts to ZERO — fire now.
            let remaining = (deadline - (self.now_fn)())
                .to_std()
                .unwrap_or(Duration::ZERO);
            if remaining.is_zero() {
                return true;
            }
            tokio::select! {
                biased;

                _ = wait_for_shutdown(shutdown) => return false,
                _ = tokio::time::sleep(remaining.min(self.max_sleep)) => {}
            }
        }
    }

    // Acquires the run lock, runs the job once, releases it; skips (no error) when
    // the lock is held or unreachable, so a backlog can't pile up. Three phases
    // around `shutdown`: acquire and release are uncancellable (they read the
    // backend reply first, so a mid-flight drop can't orphan the lock or poison a
    // pooled connection); only `job.run()` races shutdown. A panic in `run()`
    // propagates via `JoinSet` (see `supervise`); the lock still releases via Drop.
    async fn run_locked(&self, job: &dyn Job, shutdown: &mut Receiver<bool>) {
        let name = job.name();
        self.run_attempts.add(1, &[KeyValue::new("job", name)]);

        // Shutdown already signaled before we touched the lock: skip without acquiring.
        if *shutdown.borrow() {
            self.run_skipped.add(
                1,
                &[
                    KeyValue::new("job", name),
                    KeyValue::new("reason", "shutdown_before_acquire"),
                ],
            );
            return;
        }

        // Phase 1: acquire — uncancellable. Reads the SET reply before returning.
        let guard = match super::lock::acquire(name, &self.lock).await {
            super::lock::Acquired::Taken(guard) => guard,
            super::lock::Acquired::Held => {
                self.run_skipped.add(
                    1,
                    &[
                        KeyValue::new("job", name),
                        KeyValue::new("reason", "in_progress"),
                    ],
                );
                warn!(
                    job = name,
                    "Previous run still in progress; skipping this fire"
                );
                return;
            }
            super::lock::Acquired::Failed(e) => {
                self.run_skipped.add(
                    1,
                    &[
                        KeyValue::new("job", name),
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

        // Phase 2: the job — cancellable via the shutdown signal or the deadline
        // (both drop job.run()'s future, never the acquire or release around it).
        // This loop already serializes fires within the process; the deadline is
        // what bounds a hung run and keeps it from outliving its lease, which
        // would let *another* process acquire alongside it.
        let outcome = tokio::select! {
            biased;

            _ = wait_for_shutdown(shutdown) => {
                info!(job = name, "Shutdown during run; abandoning");
                "abandoned"
            }
            result = job.run() => match result {
                Ok(()) => {
                    debug!(job = name, "Scheduled job completed");
                    "ok"
                }
                Err(e) => {
                    error!(job = name, "Scheduled job failed: {e:?}");
                    "error"
                }
            },
            // Polled after the run, so a run finishing on the deadline still counts
            // as finished.
            _ = sleep_wall(self.max_run, &self.now_fn, self.max_sleep) => {
                error!(
                    job = name,
                    "Run exceeded the {:?} deadline; abandoning to keep runs from overlapping",
                    self.max_run
                );
                "timed_out"
            }
        };

        // Phase 3: release — uncancellable. Runs even after abandonment; awaiting
        // the EVAL reply costs a few ms at shutdown but leaves the lock known-free.
        guard.release().await;

        self.run_completed.add(
            1,
            &[
                KeyValue::new("job", name),
                KeyValue::new("outcome", outcome),
            ],
        );
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
    use super::super::lock::RunLock;
    use super::{validate_cron, virtual_now, DateTime, NowFn, Scheduler, Utc};

    /// Parses an RFC-3339 instant for pinning a test clock's epoch.
    fn at(rfc3339: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(rfc3339)
            .expect("test epoch must be valid RFC-3339")
            .with_timezone(&Utc)
    }
    use crate::jobs::test_support::{
        steppable_clock, AcquireOutcome, BlockingJob, CountingJob, FakeLock,
    };
    use crate::jobs::Job;
    use async_trait::async_trait;
    use opentelemetry::metrics::MeterProvider;
    use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData, ResourceMetrics};
    use opentelemetry_sdk::metrics::{InMemoryMetricExporter, SdkMeterProvider};
    use std::error::Error;
    use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};
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

    #[test]
    fn validate_cron_accepts_valid_expression() {
        assert!(validate_cron("0 0 3 * * *").is_ok());
    }

    #[test]
    fn validate_cron_rejects_invalid_expression() {
        assert!(validate_cron("not a cron expression").is_err());
    }

    #[test]
    fn validate_cron_rejects_never_firing() {
        assert!(validate_cron("0 0 3 * * * 2020").is_err());
    }

    #[test]
    fn validate_cron_rejects_five_field_expressions() {
        assert!(validate_cron("0 3 * * *").is_err());
        assert!(validate_cron("* * * * *").is_err());
    }

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
        let scheduler = Scheduler::new(virtual_now(), FakeLock::new(AcquireOutcome::Granted));
        let runner = scheduler.run_job(&schedule, &job, rx);
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

    #[tokio::test(start_paused = true)]
    async fn shutdown_during_running_job_returns_promptly() {
        let job = BlockingJob::new();
        let (tx, rx) = watch::channel(false);

        let shutdown_at: Mutex<Option<tokio::time::Instant>> = Mutex::new(None);
        let schedule = validate_cron("* * * * * *").unwrap();
        let (scheduler, provider, exporter) =
            metered_scheduler(virtual_now(), FakeLock::new(AcquireOutcome::Granted));
        let runner = scheduler.run_job(&schedule, &job, rx);
        let driver = async {
            // Wait until the job has actually started running.
            while job.started() == 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            *shutdown_at.lock().unwrap() = Some(tokio::time::Instant::now());
            let _ = tx.send(true);
        };
        tokio::join!(runner, driver);

        // Behaviour: the run started, never completed, and run_job returned in zero
        // virtual time.
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
            job.started(),
            1,
            "the job must have started before shutdown"
        );
        assert_eq!(
            job.completed(),
            0,
            "the abandoned run must not have completed"
        );

        // Metrics: abandonment records completed{abandoned} (not a skip), so the
        // attempts partition still holds.
        provider.force_flush().unwrap();
        let metrics = exporter.get_finished_metrics().unwrap();
        assert_eq!(
            counter_value(
                &metrics,
                "jobs.run.completed",
                &[("job", "blocking"), ("outcome", "abandoned")],
            ),
            1,
            "a run abandoned on shutdown must record exactly one completed{{abandoned}}"
        );
        let attempts = counter_value(&metrics, "jobs.run.attempts", &[("job", "blocking")]);
        let completed = counter_value(&metrics, "jobs.run.completed", &[("job", "blocking")]);
        let skipped = counter_value(&metrics, "jobs.run.skipped", &[("job", "blocking")]);
        assert_eq!(
            attempts,
            completed + skipped,
            "abandonment folds into completed, keeping attempts = completed + skipped"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn overlong_run_is_abandoned_at_the_deadline() {
        let job = BlockingJob::new();
        let (tx, rx) = watch::channel(false);

        let schedule = validate_cron("* * * * * *").unwrap();
        let lock = FakeLock::new(AcquireOutcome::Granted);
        let (scheduler, provider, exporter) = metered_scheduler(virtual_now(), lock.clone());
        let scheduler = scheduler.with_max_run(Duration::from_secs(2));
        let runner = scheduler.run_job(&schedule, &job, rx);
        let driver = async {
            // Two deadlines' worth: enough for the hung run to be abandoned and the
            // next fire to take the lock again.
            tokio::time::sleep(Duration::from_millis(4500)).await;
            let _ = tx.send(true);
        };
        tokio::join!(runner, driver);

        assert!(
            job.started() >= 2,
            "the deadline must free the slot so a later fire runs; started={}",
            job.started()
        );
        assert_eq!(
            job.completed(),
            0,
            "the hung runs must never have completed"
        );
        assert_eq!(
            lock.acquires(),
            lock.releases(),
            "an abandoned run must still release its lease"
        );

        provider.force_flush().unwrap();
        let metrics = exporter.get_finished_metrics().unwrap();
        assert!(
            counter_value(
                &metrics,
                "jobs.run.completed",
                &[("job", "blocking"), ("outcome", "timed_out")],
            ) >= 1,
            "an abandoned-at-deadline run must record completed{{timed_out}}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn run_job_exits_on_shutdown_without_firing() {
        let job = CountingJob::new("counting");
        let (tx, rx) = watch::channel(false);

        // Year-pinned cron (2100, the crate's max) fixes one fire far outside any
        // test run. The pin is what guarantees no fire: an unpinned "0 0 0 1 1 *"
        // recurs yearly and could fire if the paused clock lands near Jan 1 UTC.
        let schedule = validate_cron("0 0 0 1 1 * 2100").unwrap();
        let scheduler = Scheduler::new(virtual_now(), FakeLock::new(AcquireOutcome::Granted));
        let runner = scheduler.run_job(&schedule, &job, rx);
        let stopper = async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let _ = tx.send(true);
        };
        tokio::join!(runner, stopper);

        assert_eq!(
            job.runs(),
            0,
            "shutdown during sleep must exit without firing the job"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn false_shutdown_signal_does_not_stop_scheduler() {
        let job = CountingJob::new("counting");
        let (tx, rx) = watch::channel(false);

        let schedule = validate_cron("* * * * * *").unwrap();
        let scheduler = Scheduler::new(virtual_now(), FakeLock::new(AcquireOutcome::Granted));
        let runner = scheduler.run_job(&schedule, &job, rx);
        let driver = async {
            // Wait for the first fire, then send false (a no-op).
            while job.runs() == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            let _ = tx.send(false);
            // Let the scheduler run for a couple more ticks.
            tokio::time::sleep(Duration::from_millis(2500)).await;
            let _ = tx.send(true);
        };
        tokio::join!(runner, driver);

        assert!(
            job.runs() >= 2,
            "a false send must not stop the scheduler; it should keep firing"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn run_job_skips_run_when_lock_held() {
        let job = CountingJob::new("counting");
        let (tx, rx) = watch::channel(false);

        let schedule = validate_cron("* * * * * *").unwrap();
        let scheduler = Scheduler::new(virtual_now(), FakeLock::new(AcquireOutcome::Denied));
        let runner = scheduler.run_job(&schedule, &job, rx);
        let stopper = async {
            // Several ticks pass; every fire should be skipped.
            tokio::time::sleep(Duration::from_millis(3500)).await;
            let _ = tx.send(true);
        };
        tokio::join!(runner, stopper);

        assert_eq!(
            job.runs(),
            0,
            "a held lock must prevent the job from running"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn run_job_acquires_and_releases_lock_around_run() {
        let job = CountingJob::new("counting");
        let (tx, rx) = watch::channel(false);

        let schedule = validate_cron("* * * * * *").unwrap();
        let lock = FakeLock::new(AcquireOutcome::Granted);
        let scheduler = Scheduler::new(virtual_now(), lock.clone());
        let runner = scheduler.run_job(&schedule, &job, rx);
        let stopper = async {
            tokio::time::sleep(Duration::from_millis(2500)).await;
            let _ = tx.send(true);
        };
        tokio::join!(runner, stopper);

        let runs = job.runs();
        assert!(runs >= 2, "the job should have fired at least twice");
        assert_eq!(
            lock.acquires(),
            lock.releases(),
            "every acquire must be paired with a release"
        );
        assert_eq!(
            lock.releases(),
            runs,
            "the lock must be released once per run"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn attempts_partition_between_completed_and_skipped() {
        let job = CountingJob::new("counting");
        let (tx, rx) = watch::channel(false);

        let schedule = validate_cron("* * * * * *").unwrap();
        // Alternating: even fires run the job (completed{ok}), odd fires are
        // denied (skipped{in_progress}), so one run exercises both branches.
        let (scheduler, provider, exporter) =
            metered_scheduler(virtual_now(), FakeLock::new(AcquireOutcome::Alternating));
        let runner = scheduler.run_job(&schedule, &job, rx);
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

    #[tokio::test(start_paused = true)]
    async fn run_locked_fast_exits_when_already_shutdown() {
        let job = CountingJob::new("counting");
        let lock = FakeLock::new(AcquireOutcome::Granted);
        let (scheduler, provider, exporter) = metered_scheduler(virtual_now(), lock.clone());

        // Shutdown is already true when run_locked is entered.
        let (tx, mut rx) = watch::channel(false);
        tx.send(true).unwrap();
        scheduler.run_locked(&job, &mut rx).await;

        provider.force_flush().unwrap();
        let metrics = exporter.get_finished_metrics().unwrap();

        assert_eq!(
            lock.acquires(),
            0,
            "a fast-exit on shutdown must not reach acquire"
        );
        assert_eq!(
            counter_value(
                &metrics,
                "jobs.run.skipped",
                &[("job", "counting"), ("reason", "shutdown_before_acquire")],
            ),
            1,
            "the fast-exit must record one skipped{{shutdown_before_acquire}}"
        );
        assert_eq!(
            counter_value(&metrics, "jobs.run.completed", &[("job", "counting")]),
            0,
            "the fast-exit must not record a completed run"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn wall_clock_jump_forward_fires_within_max_sleep() {
        let job = CountingJob::new("counting");
        let (tx, rx) = watch::channel(false);

        // Epoch pinned on the hour, so the next hourly fire is exactly 1h out.
        let (now_fn, skew) = steppable_clock(at("2030-01-01T00:00:00Z"));
        let schedule = validate_cron("0 0 * * * *").unwrap();
        let scheduler = Scheduler::new(now_fn, FakeLock::new(AcquireOutcome::Granted))
            .with_max_sleep(Duration::from_secs(30));
        let runner = scheduler.run_job(&schedule, &job, rx);
        let driver = async {
            // Let the scheduler settle into its wait toward 01:00.
            tokio::time::sleep(Duration::from_millis(10)).await;
            assert_eq!(job.runs(), 0, "the hourly job must not fire an hour early");
            // Suspend: the wall clock lands past the fire time, tokio's does not.
            skew.store(2 * 3600, Ordering::SeqCst);
            // Two max_sleep chunks — one is enough to re-anchor and notice.
            tokio::time::sleep(Duration::from_secs(60)).await;
            let _ = tx.send(true);
        };
        tokio::join!(runner, driver);

        assert!(
            job.runs() >= 1,
            "the fire time passed during the jump; the scheduler must re-anchor to the \
             wall clock within max_sleep instead of sleeping out the monotonic delay"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn wall_clock_step_backward_does_not_refire_the_same_tick() {
        /// Steps the clock back on its first run. The step has to land between the
        /// run finishing and the scheduler re-anchoring for the next tick, and the
        /// job body *is* that window — which is also how it happens for real: NTP
        /// corrects while the job is executing.
        struct SteppingJob {
            runs: AtomicU32,
            skew: Arc<AtomicI64>,
        }
        #[async_trait]
        impl Job for SteppingJob {
            fn name(&self) -> &'static str {
                "stepping"
            }
            async fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
                if self.runs.fetch_add(1, Ordering::SeqCst) == 0 {
                    // Back 2s: `now` now sits before the 01:00:00 tick we just fired.
                    self.skew.store(-2, Ordering::SeqCst);
                }
                Ok(())
            }
        }

        // Epoch 1s before the hourly fire, so 01:00:00 lands almost immediately.
        let (now_fn, skew) = steppable_clock(at("2030-01-01T00:59:59Z"));
        let job = SteppingJob {
            runs: AtomicU32::new(0),
            skew,
        };
        let (tx, rx) = watch::channel(false);

        let schedule = validate_cron("0 0 * * * *").unwrap();
        let scheduler = Scheduler::new(now_fn, FakeLock::new(AcquireOutcome::Granted));
        let runner = scheduler.run_job(&schedule, &job, rx);
        let stopper = async {
            // Far longer than the ~2s a re-fire of 01:00:00 would take, and far
            // short of the next real fire at 02:00.
            tokio::time::sleep(Duration::from_secs(30)).await;
            let _ = tx.send(true);
        };
        tokio::join!(runner, stopper);

        assert_eq!(
            job.runs.load(Ordering::SeqCst),
            1,
            "a backward clock step must not re-fire the 01:00:00 tick; the anchor \
             must never move behind the last tick fired"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn exhausted_schedule_records_stopped_metric() {
        let job = CountingJob::new("counting");
        // No shutdown is sent: run_job returns once the schedule is exhausted.
        let (_tx, rx) = watch::channel(false);

        // Year-pinned single fire: fires once, then `after()` yields no more times.
        let schedule = validate_cron("0 0 0 1 1 * 2099").unwrap();
        let (scheduler, provider, exporter) =
            metered_scheduler(virtual_now(), FakeLock::new(AcquireOutcome::Granted));
        // Unbounded chunks: the fire is decades out, and the default 30s re-check
        // would need millions of iterations to reach it under the paused clock.
        let scheduler = scheduler.with_max_sleep(Duration::MAX);
        scheduler.run_job(&schedule, &job, rx).await;

        provider.force_flush().unwrap();
        let metrics = exporter.get_finished_metrics().unwrap();

        assert!(
            job.runs() >= 1,
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
