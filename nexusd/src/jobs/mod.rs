mod error;
mod lock;
mod scheduler;

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
/// never overlap — implementors don't manage concurrency. It imposes no timeout,
/// though, so jobs hitting external services should apply their own.
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
        let token = lock.new_token();
        // Arm the guard before acquiring so a cancel mid-acquire still releases
        // (see `scheduler::run_locked`).
        let guard = lock::LockGuard::new(job.name(), token.clone(), Arc::clone(&lock));

        match lock.acquire(job.name(), &token).await {
            Ok(true) => {}
            Ok(false) => {
                guard.disarm();
                return Err(JobError::AlreadyRunning { job: job.name() });
            }
            Err(e) => {
                guard.disarm();
                return Err(JobError::Lock(e));
            }
        }

        let result = job.run().await;
        guard.release().await;

        result.map_err(|source| JobError::Run {
            job: job.name(),
            source,
        })
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
                .run_job(name, &schedule, job.as_ref(), job_shutdown_rx)
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
    use scheduler::virtual_now;
    use std::error::Error;
    use std::sync::atomic::{AtomicU32, Ordering};
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

    /// Stub job for testing schedule resolution.
    struct StubJob;
    #[async_trait]
    impl Job for StubJob {
        fn name(&self) -> &'static str {
            "stub"
        }
        async fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
            Ok(())
        }
    }

    /// Duplicate job names must fail registry construction: lookups match
    /// first-by-name, so the second entry would be dead code.
    #[test]
    #[should_panic(expected = "duplicate names")]
    fn new_panics_on_duplicate_job_names() {
        JobRegistry::new(vec![Arc::new(StubJob), Arc::new(StubJob)]);
    }

    /// An empty registry renders the unknown-job hint as `(none)` rather than a
    /// dangling "available jobs:".
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

    /// run_on_demand validates the `[jobs.*]` config itself: a malformed cron
    /// fails before the stack is touched, giving parity with `nexusd run`.
    #[tokio::test]
    async fn run_on_demand_validates_jobs_config() {
        let registry = JobRegistry::new(vec![Arc::new(StubJob)]);
        let config = default_config_with("[jobs.stub]\ncron = \"not a cron\"\n").await;

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

    /// A `[jobs.<name>]` section that matches no registered job fails startup
    /// rather than being silently ignored.
    #[tokio::test]
    async fn scheduled_jobs_rejects_unknown_job_config_key() {
        let registry = JobRegistry::new(vec![Arc::new(StubJob)]);
        let config = default_config_with("[jobs.does_not_exist]\n").await;

        let err = registry.scheduled_jobs(&config).err().unwrap();
        assert!(
            matches!(err, JobError::UnknownJobConfig { .. }),
            "an unknown [jobs.<name>] key must fail startup, got: {err:?}"
        );
    }

    /// A malformed `[jobs.<name>]` cron is caught at startup rather than at first
    /// fire, and the error names the offending job.
    #[tokio::test]
    async fn scheduled_jobs_fails_fast_on_malformed_cron() {
        let registry = JobRegistry::new(vec![Arc::new(StubJob)]);

        let config = default_config_with("[jobs.stub]\ncron = \"not a cron\"\n").await;

        // scheduled_jobs only resolves; it never spawns. The error must name the
        // offending job so the operator needn't grep the config.
        let err = registry.scheduled_jobs(&config).err().unwrap();
        assert!(
            matches!(err, JobError::InvalidCron { ref job, .. } if job == "stub"),
            "malformed-cron error must name the section, got: {err:?}"
        );
    }

    /// A panicking job is contained: its sibling keeps running, supervise returns
    /// (no propagation or hang), and the panic is logged.
    #[tokio::test(start_paused = true)]
    #[traced_test]
    async fn panic_in_one_job_does_not_stop_siblings() {
        /// Panics on every fire.
        struct PanicJob;
        #[async_trait]
        impl Job for PanicJob {
            fn name(&self) -> &'static str {
                "panicky"
            }
            async fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
                panic!("boom");
            }
        }

        let count = Arc::new(AtomicU32::new(0));
        let cron = validate_cron("* * * * * *").unwrap();
        let jobs = vec![
            ScheduledJob {
                schedule: cron.clone(),
                job: Arc::new(PanicJob),
            },
            ScheduledJob {
                schedule: cron,
                job: Arc::new(CountingJob {
                    name: "counter",
                    count: count.clone(),
                }),
            },
        ];

        let (tx, rx) = watch::channel(false);
        let scheduler =
            scheduler::Scheduler::new(virtual_now(), Arc::new(lock::AlwaysAvailableLock));
        let supervisor = supervise(scheduler, jobs, rx);
        let stopper = async {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            let _ = tx.send(true);
        };
        // Reaching this line proves the panic was caught (supervise returned).
        let ((), ()) = tokio::join!(supervisor, stopper);

        assert!(
            count.load(Ordering::SeqCst) >= 1,
            "the sibling job must keep firing despite the panicking job"
        );
        assert!(
            logs_contain("Job scheduler panicked"),
            "the caught panic must be logged"
        );
    }

    /// A panicking job still releases its run lock via `LockGuard`'s Drop, so it
    /// doesn't stay stuck for its TTL. The panic itself still surfaces.
    #[tokio::test(start_paused = true)]
    #[traced_test]
    async fn panic_in_run_still_releases_lock() {
        use crate::jobs::error::LockResult;

        struct PanicJob;
        #[async_trait]
        impl Job for PanicJob {
            fn name(&self) -> &'static str {
                "panicky"
            }
            async fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
                panic!("boom");
            }
        }

        struct RecordingLock {
            acquired: Arc<AtomicU32>,
            released: Arc<AtomicU32>,
        }
        #[async_trait]
        impl lock::RunLock for RecordingLock {
            fn new_token(&self) -> String {
                "t".to_string()
            }
            async fn acquire(&self, _job: &str, _token: &str) -> LockResult<bool> {
                self.acquired.fetch_add(1, Ordering::SeqCst);
                Ok(true)
            }
            async fn unlock(&self, _job: &str, _token: &str) -> LockResult<()> {
                self.released.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }

        let acquired = Arc::new(AtomicU32::new(0));
        let released = Arc::new(AtomicU32::new(0));
        let cron = validate_cron("* * * * * *").unwrap();
        let jobs = vec![ScheduledJob {
            schedule: cron,
            job: Arc::new(PanicJob),
        }];

        let (tx, rx) = watch::channel(false);
        let scheduler = scheduler::Scheduler::new(
            virtual_now(),
            Arc::new(RecordingLock {
                acquired: acquired.clone(),
                released: released.clone(),
            }),
        );
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
            acquired.load(Ordering::SeqCst),
            1,
            "PanicJob dies after one fire (JoinSet catches, task ends)"
        );
        assert_eq!(
            released.load(Ordering::SeqCst),
            1,
            "the panicked run's lock must still be released via Drop"
        );
        // Existing supervise contract still holds: the panic is logged.
        assert!(
            logs_contain("Job scheduler panicked"),
            "the panic must still surface as a scheduler-internal panic"
        );
    }

    /// Minimal stub job that just counts how many times it ran.
    struct CountingJob {
        name: &'static str,
        count: Arc<AtomicU32>,
    }

    #[async_trait]
    impl Job for CountingJob {
        fn name(&self) -> &'static str {
            self.name
        }
        async fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    /// No cron configured → the job is unscheduled.
    #[test]
    fn job_cron_none_when_unscheduled() {
        assert!(job_cron(&JobConfig { cron: None }).unwrap().is_none());
    }

    /// A valid cron is parsed into a Schedule that round-trips to the same string.
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

    /// A malformed cron fails fast rather than silently going unscheduled.
    #[test]
    fn job_cron_rejects_malformed_cron() {
        let config = JobConfig {
            cron: Some("not a cron".to_string()),
        };
        assert!(job_cron(&config).is_err());
    }

    /// The scheduler fires the job on its cron. Virtual clock keeps it instant.
    #[tokio::test(start_paused = true)]
    async fn run_job_fires_on_schedule() {
        let count = Arc::new(AtomicU32::new(0));
        let job = CountingJob {
            name: "mock",
            count: count.clone(),
        };
        let (tx, rx) = watch::channel(false);

        // Fires every second; stop after ~1.5s (virtual) so it fires at least once.
        let schedule = validate_cron("* * * * * *").unwrap();
        let scheduler =
            scheduler::Scheduler::new(virtual_now(), Arc::new(lock::AlwaysAvailableLock));
        let runner = scheduler.run_job("mock", &schedule, &job, rx);
        let stopper = async {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            let _ = tx.send(true);
        };
        tokio::join!(runner, stopper);

        assert!(
            count.load(Ordering::SeqCst) >= 1,
            "a per-second cron should fire at least once"
        );
    }
}
