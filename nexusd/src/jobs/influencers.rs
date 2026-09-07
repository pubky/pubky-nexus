use async_trait::async_trait;
use nexus_common::models::user::Influencers;
use nexus_common::types::CacheTimeframe;
use std::error::Error;
use std::time::Duration;
use tokio::time::timeout;

use super::Job;

/// Per-timeframe ceiling for one graph scan + cache write. Sized to keep a
/// single scan well inside the runner's MAX_RUN deadline.
/// Monotonic on purpose: `tokio::time::timeout`, not `sleep_wall`. The
/// wall-clock discipline elsewhere exists so a run can't outlive its lease;
/// a 60s I/O window is unaffected by host suspend.
const REFRESH_TIMEOUT: Duration = Duration::from_secs(60);

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
            CacheTimeframe::Today => "influencers_cache_today",
            CacheTimeframe::ThisWeek => "influencers_cache_this_week",
            CacheTimeframe::ThisMonth => "influencers_cache_this_month",
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
                "influencers_cache_this_month",
                "influencers_cache_this_week",
                "influencers_cache_today",
            ],
            "each cache-backed timeframe must have a unique, deterministic job name"
        );
    }
}
