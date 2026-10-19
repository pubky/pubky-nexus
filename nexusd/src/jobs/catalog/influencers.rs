use async_trait::async_trait;
use nexus_common::models::user::Influencers;
use nexus_common::types::Timeframe;
use std::error::Error;
use std::time::Duration;
use tokio::time::timeout;

use crate::jobs::Job;

/// Per-timeframe ceiling for one graph scan + cache write. The scan walks every
/// user with three counting subqueries each and cannot prune early, so on a
/// large graph (or while a trust recompute loads Neo4j) it may run for minutes.
/// The cap must stay well above the API's request timeout: this job is the only
/// path that can finish a scan too slow for a request, and a timeout here writes
/// nothing. Sized generously under the runner's MAX_RUN deadline; revisit once
/// there is production timing data. A timeout is reported by the runner as
/// `JobError::Run` at ERROR level, so no extra logging is done here.
/// Monotonic on purpose: `tokio::time::timeout`, not `sleep_wall`. The
/// wall-clock discipline elsewhere exists so a run can't outlive its lease;
/// an I/O window this short is unaffected by host suspend.
const REFRESH_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Every 10 minutes, at minutes no other job uses. The AllTime ranking has no TTL
/// and no other writer, so its job runs on this unless the config says otherwise.
const ALL_TIME_CRON: &str = "0 4/10 * * * *";

/// The graph scan + cache write for one timeframe outran `REFRESH_TIMEOUT`.
#[derive(Debug, thiserror::Error)]
#[error("influencer cache refresh for {timeframe} timed out after {after:?}")]
pub struct RefreshTimedOut {
    pub timeframe: Timeframe,
    pub after: Duration,
}

/// Refresh the global influencer ranking for one timeframe on a schedule: the
/// top-100 TTL cache of a ranged timeframe, or the whole `Sorted:Users:Influencers`
/// ranking for `AllTime`. That ranking has no TTL and no other writer, so its job
/// is the one that keeps it current, on [`ALL_TIME_CRON`] by default.
pub struct InfluencersCacheJob(Timeframe);

impl InfluencersCacheJob {
    pub fn new(timeframe: Timeframe) -> Self {
        Self(timeframe)
    }
}

#[async_trait]
impl Job for InfluencersCacheJob {
    fn name(&self) -> &'static str {
        match self.0 {
            Timeframe::Today => "influencers-cache-today",
            Timeframe::ThisWeek => "influencers-cache-this-week",
            Timeframe::ThisMonth => "influencers-cache-this-month",
            Timeframe::AllTime => "influencers-cache-all-time",
        }
    }

    fn default_cron(&self) -> Option<&'static str> {
        (self.0 == Timeframe::AllTime).then_some(ALL_TIME_CRON)
    }

    /// Errors propagate typed: the runner logs them once, as `JobError::Run`.
    async fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        match timeout(REFRESH_TIMEOUT, Influencers::refresh(&self.0)).await {
            Ok(result) => Ok(result?),
            Err(_elapsed) => Err(Box::new(RefreshTimedOut {
                timeframe: self.0.clone(),
                after: REFRESH_TIMEOUT,
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn job_names_are_distinct_per_timeframe() {
        let registry = crate::jobs::JobRegistry::new(vec![
            Arc::new(InfluencersCacheJob::new(Timeframe::Today)),
            Arc::new(InfluencersCacheJob::new(Timeframe::ThisWeek)),
            Arc::new(InfluencersCacheJob::new(Timeframe::ThisMonth)),
            Arc::new(InfluencersCacheJob::new(Timeframe::AllTime)),
        ]);

        let mut names = registry.job_names();
        names.sort_unstable();
        assert_eq!(
            names,
            vec![
                "influencers-cache-all-time",
                "influencers-cache-this-month",
                "influencers-cache-this-week",
                "influencers-cache-today",
            ],
            "each timeframe must have a unique, deterministic job name"
        );
    }
}
