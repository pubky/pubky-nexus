use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use tokio::sync::watch::Receiver;
use tracing::{error, warn};

use super::{
    lock::{self, LockMetrics, RedisRunLock},
    registry::ScheduledJob,
    scheduler::{self, NowFn},
    Job, JobError,
};

/// Runs `job` once under `lock`, mapping lock state to [`JobError`]. Used by
/// [`crate::jobs::JobRegistry::run_on_demand`]; `lock`, `now_fn`, and `metrics`
/// are injected so tests don't need a stack.
///
/// Abandoned at [`lock::MAX_RUN`] like a scheduled run, so an on-demand run can't
/// outlive its lease either.
pub(super) async fn run_once_locked(
    job: &dyn Job,
    lock: &Arc<dyn lock::RunLock>,
    now_fn: &NowFn,
    metrics: &LockMetrics,
) -> Result<(), JobError> {
    let guard = match lock::acquire(job.name(), lock, metrics).await {
        lock::Acquired::Taken(guard) => guard,
        lock::Acquired::Held => return Err(JobError::AlreadyRunning { job: job.name() }),
        lock::Acquired::Failed { error, released } => {
            if !released {
                warn!(
                    job = job.name(),
                    "Lock acquire failed and inline release also failed — slot may stay held until TTL expires"
                );
            }
            return Err(JobError::Lock(error));
        }
        lock::Acquired::TimedOut { released } => {
            if !released {
                warn!(
                    job = job.name(),
                    "Lock acquire timed out and inline release also failed — slot may stay held until TTL expires"
                );
            }
            return Err(JobError::LockTimedOut { job: job.name() });
        }
    };

    // Wall-clock deadline, not `tokio::time::timeout` — see `scheduler::sleep_wall`.
    let result = tokio::select! {
        biased;

        // Polled first, so a run finishing on the deadline still counts as finished.
        result = job.run() => Some(result),
        _ = scheduler::sleep_wall(lock::MAX_RUN, now_fn, scheduler::MAX_SLEEP) => None,
    };
    let release_outcome = guard.release().await;
    if !matches!(
        release_outcome,
        lock::ReleaseOutcome::Released | lock::ReleaseOutcome::NotHeld
    ) {
        warn!(
            job = job.name(),
            ?release_outcome,
            "Lock release failed after run — slot may stay held until TTL expires"
        );
    }

    match result {
        Some(Ok(())) => Ok(()),
        Some(Err(source)) => Err(JobError::Run {
            job: job.name(),
            source,
        }),
        None => Err(JobError::TimedOut {
            job: job.name(),
            after: lock::MAX_RUN,
        }),
    }
}

/// Runs every scheduled job until shutdown, one supervised task per job. A panic
/// in one job is caught and logged, leaving siblings up until restart. Returns
/// once all jobs stop (immediately when there are none). Sets up the stack
/// before spawning, so a fast cron can't outrace it to `StackManager::setup`.
pub async fn run(
    jobs: Vec<ScheduledJob>,
    stack: &nexus_common::StackConfig,
    shutdown_rx: Receiver<bool>,
) -> Result<(), JobError> {
    if jobs.is_empty() {
        return Ok(());
    }
    nexus_common::StackManager::setup(stack)
        .await
        .map_err(JobError::Stack)?;
    let scheduler = scheduler::Scheduler::new(
        Arc::new(Utc::now) as scheduler::NowFn,
        Arc::new(RedisRunLock::new()),
    );
    supervise(scheduler, jobs, shutdown_rx).await;
    Ok(())
}

/// Spawns and supervises one task per job until shutdown, catching and logging a
/// per-job panic (see [`run`]). Split from `run` so supervision is testable
/// without the stack; `scheduler` (clock + lock) is injected for the same reason.
async fn supervise(
    scheduler: scheduler::Scheduler,
    jobs: Vec<ScheduledJob>,
    shutdown_rx: Receiver<bool>,
) {
    let mut set = tokio::task::JoinSet::new();
    // Map task id -> job name so we can recover the name on panic.
    let mut names: HashMap<tokio::task::Id, &'static str> = HashMap::new();

    for ScheduledJob { schedule, job } in jobs {
        let name = job.name();
        let job_shutdown_rx = shutdown_rx.clone();
        let scheduler = scheduler.clone();
        let handle = set.spawn(async move {
            scheduler
                .run_job(&schedule, job.as_ref(), job_shutdown_rx)
                .await;
        });
        names.insert(handle.id(), name);
    }

    while let Some(result) = set.join_next_with_id().await {
        match result {
            Ok((id, ())) => {
                names.remove(&id);
            }
            Err(e) => {
                let name = names.remove(&e.id()).unwrap_or("<unknown>");
                scheduler.record_stopped(name, "panic");
                error!(
                    job = name,
                    "Job scheduler panicked; disabled until restart: {e}"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::lock::LockMetrics;
    use super::*;
    use crate::jobs::{
        scheduler::{self, virtual_now},
        test_support::{
            AcquireOutcome, BlockingJob, CountingJob, FakeLock, PanicJob, UnlockOutcome,
        },
        validate_cron, JobError,
    };
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::watch;
    use tracing_test::traced_test;

    #[tokio::test]
    async fn run_once_locked_reports_a_held_lock_as_already_running() {
        let lock: Arc<dyn lock::RunLock> =
            FakeLock::new(AcquireOutcome::Denied, UnlockOutcome::Succeeds);
        let job = CountingJob::new("stub");
        let err = run_once_locked(&job, &lock, &virtual_now(), &LockMetrics::new())
            .await
            .err()
            .unwrap();

        assert!(
            matches!(err, JobError::AlreadyRunning { job } if job == "stub"),
            "a held lock must surface as AlreadyRunning, got: {err:?}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn run_once_locked_abandons_a_run_past_the_deadline() {
        let job = BlockingJob::new();
        let lock: Arc<dyn lock::RunLock> =
            FakeLock::new(AcquireOutcome::Granted, UnlockOutcome::Succeeds);

        // Paused time auto-advances while the run hangs, so the hour costs nothing.
        let err = run_once_locked(&job, &lock, &virtual_now(), &LockMetrics::new())
            .await
            .err()
            .unwrap();

        assert!(
            matches!(err, JobError::TimedOut { job, .. } if job == "blocking"),
            "a run past the deadline must surface as TimedOut, got: {err:?}"
        );
        assert_eq!(
            job.completed(),
            0,
            "the abandoned run must not have completed"
        );
    }

    #[tokio::test]
    async fn run_once_locked_surfaces_lock_errors() {
        let job = CountingJob::new("counter");
        let lock: Arc<dyn lock::RunLock> =
            FakeLock::new(AcquireOutcome::Fails, UnlockOutcome::Succeeds);
        let err = run_once_locked(&job, &lock, &virtual_now(), &LockMetrics::new())
            .await
            .err()
            .unwrap();

        assert!(
            matches!(err, JobError::Lock(_)),
            "an unreachable backend must surface as JobError::Lock, got: {err:?}"
        );
        assert_eq!(
            job.runs(),
            0,
            "the job must not run when the lock could not be taken"
        );
    }

    #[tokio::test(start_paused = true)]
    #[traced_test]
    async fn panic_in_one_job_does_not_stop_siblings() {
        let counter = Arc::new(CountingJob::new("counter"));
        let cron = validate_cron("* * * * * *").unwrap();
        let jobs = vec![
            ScheduledJob {
                schedule: cron.clone(),
                job: Arc::new(PanicJob),
            },
            ScheduledJob {
                schedule: cron,
                job: counter.clone(),
            },
        ];

        let (tx, rx) = watch::channel(false);
        let scheduler = scheduler::Scheduler::new(
            virtual_now(),
            FakeLock::new(AcquireOutcome::Granted, UnlockOutcome::Succeeds),
        );
        let supervisor = supervise(scheduler, jobs, rx);
        let stopper = async {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            let _ = tx.send(true);
        };
        // Reaching this line proves the panic was caught (supervise returned).
        let ((), ()) = tokio::join!(supervisor, stopper);

        assert!(
            counter.runs() >= 1,
            "the sibling job must keep firing despite the panicking job"
        );
        assert!(
            logs_contain("Job scheduler panicked"),
            "the caught panic must be logged"
        );
    }

    #[tokio::test(start_paused = true)]
    #[traced_test]
    async fn panic_in_run_still_releases_lock() {
        let cron = validate_cron("* * * * * *").unwrap();
        let jobs = vec![ScheduledJob {
            schedule: cron,
            job: Arc::new(PanicJob),
        }];

        let (tx, rx) = watch::channel(false);
        let lock = FakeLock::new(AcquireOutcome::Granted, UnlockOutcome::Succeeds);
        let scheduler = scheduler::Scheduler::new(virtual_now(), lock.clone());
        let supervisor = supervise(scheduler, jobs, rx);
        let stopper = async {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            // Yield after shutdown so any Drop-spawned unlock tasks get a chance to run.
            let _ = tx.send(true);
        };
        let ((), ()) = tokio::join!(supervisor, stopper);

        // Let detached Drop-spawn tasks complete before asserting.
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(10)).await;

        assert_eq!(
            lock.acquires(),
            1,
            "PanicJob dies after one fire (JoinSet catches, task ends)"
        );
        assert_eq!(
            lock.unlock_attempts(),
            1,
            "the panicked run's lock must still be released via Drop"
        );
        // Existing supervise contract still holds: the panic is logged.
        assert!(
            logs_contain("Job scheduler panicked"),
            "the panic must still surface as a scheduler-internal panic"
        );
    }

    #[tokio::test(start_paused = true)]
    #[traced_test]
    async fn run_once_locked_warns_when_acquire_times_out_and_release_fails() {
        let job = CountingJob::new("stub");
        // Hangs + Hangs => acquire times out, inline release also times out (released == false)
        let lock: Arc<dyn lock::RunLock> =
            FakeLock::new(AcquireOutcome::Hangs, UnlockOutcome::Hangs);

        let err = run_once_locked(&job, &lock, &virtual_now(), &LockMetrics::new())
            .await
            .err()
            .unwrap();

        assert!(
            matches!(err, JobError::LockTimedOut { job } if job == "stub"),
            "got: {err:?}"
        );
        assert!(
            logs_contain("Lock acquire timed out and inline release also failed"),
            "should warn when both acquire timeout and inline release fail"
        );
    }

    #[tokio::test(start_paused = true)]
    #[traced_test]
    async fn run_once_locked_warns_when_acquire_fails_and_release_fails() {
        let job = CountingJob::new("stub");
        // Fails + Hangs => acquire errors, inline release times out (released == false)
        let lock: Arc<dyn lock::RunLock> =
            FakeLock::new(AcquireOutcome::Fails, UnlockOutcome::Hangs);

        let err = run_once_locked(&job, &lock, &virtual_now(), &LockMetrics::new())
            .await
            .err()
            .unwrap();

        assert!(matches!(err, JobError::Lock(_)), "got: {err:?}");
        assert!(
            logs_contain("Lock acquire failed and inline release also failed"),
            "should warn when both acquire error and inline release fail"
        );
    }

    #[tokio::test(start_paused = true)]
    #[traced_test]
    async fn run_once_locked_warns_when_phase_three_release_fails() {
        let job = CountingJob::new("stub");
        // Granted + Hangs => acquire succeeds, phase-3 release times out
        let lock: Arc<dyn lock::RunLock> =
            FakeLock::new(AcquireOutcome::Granted, UnlockOutcome::Hangs);

        let result = run_once_locked(&job, &lock, &virtual_now(), &LockMetrics::new()).await;
        assert!(
            result.is_ok(),
            "the run itself must succeed; only the phase-3 release hangs, got: {result:?}"
        );

        assert!(
            logs_contain("Lock release failed after run"),
            "should warn when phase-3 release fails"
        );
    }
}
