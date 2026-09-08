use async_trait::async_trait;
use nexus_common::models::user::Influencers;
use nexus_common::types::CacheTimeframe;
use std::error::Error;
use std::time::Duration;
use tokio::time::timeout;

use super::Job;

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

/// The graph scan + cache write for one timeframe outran `REFRESH_TIMEOUT`.
#[derive(Debug, thiserror::Error)]
#[error("influencer cache refresh for {timeframe} timed out after {after:?}")]
pub struct RefreshTimedOut {
    pub timeframe: CacheTimeframe,
    pub after: Duration,
}

/// Refresh the global influencer cache for one `CacheTimeframe` on a schedule.
///
/// `Timeframe::AllTime` is not constructible here: it is served from the
/// incrementally maintained `Sorted:Users:Influencers` set, has no cache key,
/// and a job for it would only ever write an orphan.
pub struct InfluencersCacheJob(CacheTimeframe);

impl InfluencersCacheJob {
    pub fn new(timeframe: CacheTimeframe) -> Self {
        Self(timeframe)
    }
}

#[async_trait]
impl Job for InfluencersCacheJob {
    fn name(&self) -> &'static str {
        match self.0 {
            CacheTimeframe::Today => "influencers-cache-today",
            CacheTimeframe::ThisWeek => "influencers-cache-this-week",
            CacheTimeframe::ThisMonth => "influencers-cache-this-month",
        }
    }

    /// Errors propagate typed: the runner logs them once, as `JobError::Run`.
    async fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        match timeout(REFRESH_TIMEOUT, Influencers::fetch_and_cache(self.0)).await {
            Ok(result) => Ok(result?),
            Err(_elapsed) => Err(Box::new(RefreshTimedOut {
                timeframe: self.0,
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
        let registry = super::super::JobRegistry::new(vec![
            Arc::new(InfluencersCacheJob::new(CacheTimeframe::Today)),
            Arc::new(InfluencersCacheJob::new(CacheTimeframe::ThisWeek)),
            Arc::new(InfluencersCacheJob::new(CacheTimeframe::ThisMonth)),
        ]);

        let mut names = registry.job_names();
        names.sort_unstable();
        assert_eq!(
            names,
            vec![
                "influencers-cache-this-month",
                "influencers-cache-this-week",
                "influencers-cache-today",
            ],
            "each cache-backed timeframe must have a unique, deterministic job name"
        );
    }
}
