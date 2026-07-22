mod error;
mod lock;
mod scheduler;
#[cfg(test)]
mod test_support;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use cron::Schedule;
use nexus_common::{DaemonConfig, JobConfig};
use tokio::sync::watch::Receiver;
use tracing::error;

pub use error::{CronParseError, JobError};
use lock::RedisRunLock;
pub use lock::LOCK_TTL_SECS;
pub use scheduler::validate_cron;

/// Resolves and validates a job's cron: `None` when unscheduled, else the
/// parsed [`Schedule`] so callers don't re-parse.
pub fn job_cron(config: &JobConfig) -> Result<Option<Schedule>, CronParseError> {
    let Some(cron) = &config.cron else {
        return Ok(None);
    };
    Ok(Some(validate_cron(cron)?))
}

/// A unit of work, runnable on demand or on a schedule. Its name matches the
/// `[jobs.<name>]` config section. The runner and scheduler know nothing about
/// any concrete job; both set up the stack first, so `run` can assume it's up.
///
/// The runner takes a cross-process run lock around every run, so a job's runs
/// never overlap — implementors don't manage concurrency. A run is abandoned at
/// the runner's one-hour deadline, so a job needing finer granularity should
/// apply its own timeouts. Abandonment only drops `run`'s future: work the job
/// spawned onto its own task keeps going, outside the lock's protection.
#[async_trait]
pub trait Job: Send + Sync {
    /// Stable unique identifier: used in logs, on-demand selection, and as the
    /// `[jobs.<name>]` config key.
    fn name(&self) -> &'static str;

    /// Executes a single run. A returned error is logged but doesn't stop future
    /// scheduled runs.
    async fn run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// A [`Job`] bound to its cron schedule, with the [`Schedule`] pre-parsed. `Arc`
/// shares one instance between the registry and the scheduler.
pub struct ScheduledJob {
    schedule: Schedule,
    job: Arc<dyn Job>,
}

/// The single source of truth for what jobs exist — listing, on-demand runs, and
/// scheduling all read from here.
pub struct JobRegistry {
    jobs: Vec<Arc<dyn Job>>,
}

impl JobRegistry {
    /// Builds a registry from pre-built jobs. Panics on duplicate names, since
    /// lookups match first-by-name and the second entry would be dead code.
    pub fn new(jobs: Vec<Arc<dyn Job>>) -> Self {
        let mut seen = HashSet::new();
        assert!(
            jobs.iter().all(|j| seen.insert(j.name())),
            "registry contains jobs with duplicate names"
        );
        Self { jobs }
    }

    /// Names of all available jobs; used for `jobs list` and error messages.
    pub fn job_names(&self) -> Vec<&'static str> {
        self.jobs.iter().map(|job| job.name()).collect()
    }

    /// Job list for error hints; `(none)` when empty.
    fn available_jobs_hint(&self) -> String {
        let names = self.job_names();
        if names.is_empty() {
            "(none)".to_string()
        } else {
            names.join(", ")
        }
    }

    /// Runs a single job once, even when unscheduled. Validates the full
    /// `[jobs.*]` config up front (parity with `nexusd run`) and sets up the
    /// stack before running.
    ///
    /// Takes the scheduler's cross-process run lock, so it can't overlap a
    /// scheduled run. Unlike the scheduler (which skips), a held lock is an error
    /// here: the operator asked to run *now*.
    pub async fn run_on_demand(&self, name: &str, config: &DaemonConfig) -> Result<(), JobError> {
        self.scheduled_jobs(config)?;
        let job = self
            .jobs
            .iter()
            .find(|job| job.name() == name)
            .ok_or_else(|| JobError::UnknownJobName {
                name: name.into(),
                available: self.available_jobs_hint(),
            })?;
        nexus_common::StackManager::setup(&config.stack)
            .await
            .map_err(JobError::Stack)?;

        let lock: Arc<dyn lock::RunLock> = Arc::new(RedisRunLock::new());
        let now_fn = Arc::new(Utc::now) as scheduler::NowFn;
        run_once_locked(job.as_ref(), &lock, &now_fn).await
    }

    /// The scheduled jobs, resolved from config. Each job's schedule is validated
    /// here, so a misconfigured job fails fast rather than at its first fire.
    pub fn scheduled_jobs(&self, config: &DaemonConfig) -> Result<Vec<ScheduledJob>, JobError> {
        let job_names = self.job_names();

        // Fail fast on a `[jobs.<name>]` section matching no registered job (typo).
        let mut unknown: Vec<&str> = config
            .jobs
            .keys()
            .map(String::as_str)
            .filter(|name| !job_names.contains(name))
            .collect();
        if !unknown.is_empty() {
            unknown.sort_unstable();
            return Err(JobError::UnknownJobConfig {
                unknown: unknown.into_iter().map(String::from).collect(),
                available: self.available_jobs_hint(),
            });
        }

        let mut jobs = Vec::new();

        for job in &self.jobs {
            let name = job.name();
            // An absent `[jobs.<name>]` section means the job is unscheduled.
            let job_config = config.jobs.get(name).cloned().unwrap_or_default();
            // Tag the error with the section naming the malformed cron.
            let cron = job_cron(&job_config).map_err(|source| JobError::InvalidCron {
                job: name.into(),
                source,
            })?;
            if let Some(schedule) = cron {
                jobs.push(ScheduledJob {
                    schedule,
                    job: Arc::clone(job),
                });
            }
        }

        Ok(jobs)
    }
}

/// Runs `job` once under `lock`, mapping lock state to [`JobError`]. Split from
/// [`JobRegistry::run_on_demand`] so it's testable without a stack; `lock` and
/// `now_fn` are injected for the same reason.
///
/// Abandoned at [`lock::MAX_RUN`] like a scheduled run, so an on-demand run can't
/// outlive its lease either.
async fn run_once_locked(
    job: &dyn Job,
    lock: &Arc<dyn lock::RunLock>,
    now_fn: &scheduler::NowFn,
) -> Result<(), JobError> {
    let guard = match lock::acquire(job.name(), lock).await {
        lock::Acquired::Taken(guard) => guard,
        lock::Acquired::Held => return Err(JobError::AlreadyRunning { job: job.name() }),
        lock::Acquired::Failed(e) => return Err(JobError::Lock(e)),
    };

    // Wall-clock deadline, not `tokio::time::timeout` — see `scheduler::sleep_wall`.
    let result = tokio::select! {
        biased;

        // Polled first, so a run finishing on the deadline still counts as finished.
        result = job.run() => Some(result),
        _ = scheduler::sleep_wall(lock::MAX_RUN, now_fn, scheduler::MAX_SLEEP) => None,
    };
    guard.release().await;

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
    use super::*;
    use crate::jobs::test_support::{AcquireOutcome, CountingJob, FakeLock, PanicJob};
    use scheduler::virtual_now;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::watch;
    use tracing_test::traced_test;

    /// Builds a [`DaemonConfig`] from the canonical default config with `extra`
    /// TOML appended (e.g. a `[jobs.<name>]` section).
    async fn default_config_with(extra: &str) -> DaemonConfig {
        use nexus_common::file::{ConfigLoader, CONFIG_FILE_NAME};

        let dir = tempfile::TempDir::new().unwrap();
        DaemonConfig::read_or_create_config_file(dir.path().to_path_buf())
            .await
            .unwrap();
        let default_toml = std::fs::read_to_string(dir.path().join(CONFIG_FILE_NAME)).unwrap();
        DaemonConfig::try_from_str(&format!("{default_toml}\n{extra}"))
            .expect("config with the appended section should parse")
    }

    /// A registry with `extra` plus a stub for every other `[jobs.<name>]`
    /// section `config` carries, so validation against the full default config
    /// sees a known job per shipped section without this generic test naming a
    /// specific one.
    fn registry_covering(config: &DaemonConfig, extra: &'static str) -> JobRegistry {
        let mut jobs: Vec<Arc<dyn Job>> = vec![Arc::new(CountingJob::new(extra))];
        for name in config.jobs.keys() {
            if name != extra {
                let name: &'static str = Box::leak(name.clone().into_boxed_str());
                jobs.push(Arc::new(CountingJob::new(name)));
            }
        }
        JobRegistry::new(jobs)
    }

    #[tokio::test]
    async fn run_once_locked_reports_a_held_lock_as_already_running() {
        let lock: Arc<dyn lock::RunLock> = FakeLock::new(AcquireOutcome::Denied);
        let job = CountingJob::new("stub");
        let err = run_once_locked(&job, &lock, &virtual_now())
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
        let job = crate::jobs::test_support::BlockingJob::new();
        let lock: Arc<dyn lock::RunLock> = FakeLock::new(AcquireOutcome::Granted);

        // Paused time auto-advances while the run hangs, so the hour costs nothing.
        let err = run_once_locked(&job, &lock, &virtual_now())
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
        let lock: Arc<dyn lock::RunLock> = FakeLock::new(AcquireOutcome::Fails);
        let err = run_once_locked(&job, &lock, &virtual_now())
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

    #[test]
    #[should_panic(expected = "duplicate names")]
    fn new_panics_on_duplicate_job_names() {
        JobRegistry::new(vec![
            Arc::new(CountingJob::new("stub")),
            Arc::new(CountingJob::new("stub")),
        ]);
    }

    #[tokio::test]
    async fn unknown_job_error_shows_none_when_registry_empty() {
        let registry = JobRegistry::new(Vec::new());
        let config = default_config_with("").await;

        // The name lookup fails before stack setup, so this needs no stack.
        let err = match registry.run_on_demand("whatever", &config).await {
            Ok(()) => panic!("running against an empty registry must error"),
            Err(e) => e.to_string(),
        };

        assert!(
            err.contains("available jobs: (none)"),
            "empty registry must render `(none)`, got: {err}"
        );
    }

    #[tokio::test]
    async fn run_on_demand_validates_jobs_config() {
        let config = default_config_with("[jobs.stub]\ncron = \"not a cron\"\n").await;
        let registry = registry_covering(&config, "stub");

        // The bad cron is caught before StackManager::setup, so no stack is needed.
        let err = match registry.run_on_demand("stub", &config).await {
            Ok(()) => panic!("a malformed cron must fail run_on_demand"),
            Err(e) => e.to_string(),
        };
        assert!(
            err.contains("[jobs.stub]"),
            "run_on_demand must surface the config error naming the section, got: {err}"
        );
    }

    #[tokio::test]
    async fn scheduled_jobs_rejects_unknown_job_config_key() {
        let registry = JobRegistry::new(vec![Arc::new(CountingJob::new("stub"))]);
        let config = default_config_with("[jobs.does_not_exist]\n").await;

        let err = registry.scheduled_jobs(&config).err().unwrap();
        assert!(
            matches!(err, JobError::UnknownJobConfig { .. }),
            "an unknown [jobs.<name>] key must fail startup, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn scheduled_jobs_fails_fast_on_malformed_cron() {
        let config = default_config_with("[jobs.stub]\ncron = \"not a cron\"\n").await;
        let registry = registry_covering(&config, "stub");

        // scheduled_jobs only resolves; it never spawns. The error must name the
        // offending job so the operator needn't grep the config.
        let err = registry.scheduled_jobs(&config).err().unwrap();
        assert!(
            matches!(err, JobError::InvalidCron { ref job, .. } if job == "stub"),
            "malformed-cron error must name the section, got: {err:?}"
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
        let scheduler =
            scheduler::Scheduler::new(virtual_now(), FakeLock::new(AcquireOutcome::Granted));
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
        let lock = FakeLock::new(AcquireOutcome::Granted);
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
            lock.releases(),
            1,
            "the panicked run's lock must still be released via Drop"
        );
        // Existing supervise contract still holds: the panic is logged.
        assert!(
            logs_contain("Job scheduler panicked"),
            "the panic must still surface as a scheduler-internal panic"
        );
    }

    #[test]
    fn job_cron_none_when_unscheduled() {
        assert!(job_cron(&JobConfig { cron: None }).unwrap().is_none());
    }

    #[test]
    fn job_cron_returns_valid_cron() {
        let config = JobConfig {
            cron: Some("0 0 3 * * *".to_string()),
        };
        let schedule = job_cron(&config).unwrap().unwrap();
        // The Schedule retains its source expression verbatim.
        assert_eq!(
            schedule.to_string(),
            "0 0 3 * * *",
            "parsed schedule must round-trip the input cron expression"
        );
    }

    #[test]
    fn job_cron_rejects_malformed_cron() {
        let config = JobConfig {
            cron: Some("not a cron".to_string()),
        };
        assert!(job_cron(&config).is_err());
    }

    #[tokio::test(start_paused = true)]
    async fn run_job_fires_on_schedule() {
        let job = CountingJob::new("mock");
        let (tx, rx) = watch::channel(false);

        // Fires every second; stop after ~1.5s (virtual) so it fires at least once.
        let schedule = validate_cron("* * * * * *").unwrap();
        let scheduler =
            scheduler::Scheduler::new(virtual_now(), FakeLock::new(AcquireOutcome::Granted));
        let runner = scheduler.run_job(&schedule, &job, rx);
        let stopper = async {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            let _ = tx.send(true);
        };
        tokio::join!(runner, stopper);

        assert!(
            job.runs() >= 1,
            "a per-second cron should fire at least once"
        );
    }
}
