mod error;
mod hot_tags;
mod influencers;
mod lock;
mod registry;
mod runner;
mod scheduler;
#[cfg(test)]
mod test_support;

pub use error::{CronParseError, JobError};
pub use hot_tags::HotTagsCacheJob;
pub use influencers::InfluencersCacheJob;
pub use lock::LOCK_TTL_SECS;
pub use registry::{JobRegistry, ScheduledJob};
pub use runner::run;
pub use scheduler::validate_cron;

use async_trait::async_trait;
use cron::Schedule;
use nexus_common::JobConfig;

/// OpenTelemetry meter name for all job metrics.
const METER_NAME: &str = "nexus.jobs";

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

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_common::JobConfig;

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
}
