use async_trait::async_trait;
use nexus_common::models::error::ModelError;
use nexus_common::models::user::Influencers;
use nexus_common::types::Timeframe;
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

/// Refresh the global influencer cache for one `Timeframe` on a schedule.
///
/// `AllTime` is intentionally not represented: it is served from the
/// incrementally maintained `Sorted:Users:Influencers` set and has no TTL.
pub struct InfluencersCacheJob(pub Timeframe);

#[async_trait]
impl Job for InfluencersCacheJob {
    fn name(&self) -> &'static str {
        match self.0 {
            Timeframe::Today => "influencers_cache_today",
            Timeframe::ThisWeek => "influencers_cache_this_week",
            Timeframe::ThisMonth => "influencers_cache_this_month",
            // Never registered; included only to keep the match exhaustive.
            Timeframe::AllTime => "influencers_cache_all_time",
        }
    }

    async fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        Influencers::refresh_timeframes_with(std::slice::from_ref(&self.0), &|tf| async move {
            match timeout(REFRESH_TIMEOUT, Influencers::fetch_and_cache(&tf)).await {
                Ok(result) => result,
                Err(_elapsed) => Err(ModelError::from_generic(format!(
                    "{tf} refresh timed out after {REFRESH_TIMEOUT:?}"
                ))),
            }
        })
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn job_names_are_distinct_per_timeframe() {
        let registry = super::super::JobRegistry::new(vec![
            Arc::new(InfluencersCacheJob(Timeframe::Today)),
            Arc::new(InfluencersCacheJob(Timeframe::ThisWeek)),
            Arc::new(InfluencersCacheJob(Timeframe::ThisMonth)),
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
