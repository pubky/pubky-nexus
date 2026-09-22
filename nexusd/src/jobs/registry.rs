use std::collections::HashSet;
use std::sync::Arc;

use chrono::Utc;
use cron::Schedule;
use nexus_common::DaemonConfig;

use super::{
    job_cron,
    lock::{self, LockMetrics, RedisRunLock},
    runner, scheduler, Job, JobError,
};

/// A [`Job`] bound to its cron schedule, with the [`Schedule`] pre-parsed. `Arc`
/// shares one instance between the registry and the scheduler.
pub(crate) struct ScheduledJob {
    pub(super) schedule: Schedule,
    pub(super) job: Arc<dyn Job>,
}

/// Jobs this process can run. The daemon uses [`Self::catalog`]; tests use [`Self::new`].
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
        let metrics = LockMetrics::new();
        runner::run_once_locked(job.as_ref(), &lock, &now_fn, &metrics).await
    }

    /// The scheduled jobs, resolved from config. Each job's schedule is validated
    /// here, so a misconfigured job fails fast rather than at its first fire.
    pub(crate) fn scheduled_jobs(
        &self,
        config: &DaemonConfig,
    ) -> Result<Vec<ScheduledJob>, JobError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::{test_support::CountingJob, HotTagsCacheJob, InfluencersCacheJob, JobError};
    use nexus_common::types::{CacheTimeframe, Timeframe};
    use std::sync::Arc;

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
    async fn scheduled_jobs_resolves_per_timeframe_influencer_cadences() {
        let extra = "\
[jobs.influencers-cache-today]\n\
cron = \"0 7,37 * * * *\"\n\
[jobs.influencers-cache-this-week]\n\
cron = \"0 17 */3 * * *\"\n\
[jobs.influencers-cache-this-month]\n\
cron = \"0 27 3,15 * * *\"\n";
        let config = default_config_with(extra).await;
        let registry = JobRegistry::new(vec![
            Arc::new(InfluencersCacheJob::new(CacheTimeframe::Today)),
            Arc::new(InfluencersCacheJob::new(CacheTimeframe::ThisWeek)),
            Arc::new(InfluencersCacheJob::new(CacheTimeframe::ThisMonth)),
            Arc::new(CountingJob::new("trust-recompute")),
        ]);

        let jobs = registry
            .scheduled_jobs(&config)
            .expect("per-timeframe influencer crons should resolve");

        let mut by_name: std::collections::HashMap<&str, String> = jobs
            .iter()
            .map(|sj| (sj.job.name(), sj.schedule.to_string()))
            .collect();

        assert_eq!(
            by_name.len(),
            3,
            "each cache-backed timeframe must become its own scheduled job"
        );
        assert_eq!(
            by_name.remove("influencers-cache-today").unwrap(),
            "0 7,37 * * * *"
        );
        assert_eq!(
            by_name.remove("influencers-cache-this-week").unwrap(),
            "0 17 */3 * * *"
        );
        assert_eq!(
            by_name.remove("influencers-cache-this-month").unwrap(),
            "0 27 3,15 * * *"
        );
    }

    #[tokio::test]
    async fn scheduled_jobs_resolves_per_timeframe_hot_tags_cadences() {
        let extra = "\
[jobs.hot-tags-cache-today]\n\
cron = \"0 12,42 * * * *\"\n\
[jobs.hot-tags-cache-this-week]\n\
cron = \"0 22 */3 * * *\"\n\
[jobs.hot-tags-cache-this-month]\n\
cron = \"0 32 3,15 * * *\"\n\
[jobs.hot-tags-cache-all-time]\n\
cron = \"0 47 3,15 * * *\"\n";
        let config = default_config_with(extra).await;
        let registry = JobRegistry::new(vec![
            Arc::new(HotTagsCacheJob::new(Timeframe::Today)),
            Arc::new(HotTagsCacheJob::new(Timeframe::ThisWeek)),
            Arc::new(HotTagsCacheJob::new(Timeframe::ThisMonth)),
            Arc::new(HotTagsCacheJob::new(Timeframe::AllTime)),
            Arc::new(CountingJob::new("trust-recompute")),
        ]);

        let jobs = registry
            .scheduled_jobs(&config)
            .expect("per-timeframe hot-tags crons should resolve");

        let mut by_name: std::collections::HashMap<&str, String> = jobs
            .iter()
            .map(|sj| (sj.job.name(), sj.schedule.to_string()))
            .collect();

        assert_eq!(
            by_name.len(),
            4,
            "each cache-backed timeframe must become its own scheduled job"
        );
        assert_eq!(
            by_name.remove("hot-tags-cache-today").unwrap(),
            "0 12,42 * * * *"
        );
        assert_eq!(
            by_name.remove("hot-tags-cache-this-week").unwrap(),
            "0 22 */3 * * *"
        );
        assert_eq!(
            by_name.remove("hot-tags-cache-this-month").unwrap(),
            "0 32 3,15 * * *"
        );
        assert_eq!(
            by_name.remove("hot-tags-cache-all-time").unwrap(),
            "0 47 3,15 * * *"
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
}
