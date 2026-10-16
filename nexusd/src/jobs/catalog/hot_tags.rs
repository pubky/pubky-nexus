use async_trait::async_trait;
use nexus_common::models::tag::stream::HotTags;
use nexus_common::types::Timeframe;
use std::error::Error;
use std::time::Duration;
use tokio::time::timeout;

use crate::jobs::Job;

/// Cap for one scan + write. Must outlast the API request timeout (a miss cannot
/// finish a scan this long) and stay under the runner's MAX_RUN. `tokio::time::timeout`
/// on purpose: this is an I/O window, not the wall-clock lease.
const REFRESH_TIMEOUT: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, thiserror::Error)]
#[error("hot tags cache refresh for {timeframe} timed out after {after:?}")]
struct RefreshTimedOut {
    timeframe: Timeframe,
    after: Duration,
}

/// Refresh the global hot-tags cache for one timeframe. All four [`Timeframe`]s
/// are cache-backed, including `AllTime`.
pub struct HotTagsCacheJob(Timeframe);

impl HotTagsCacheJob {
    pub fn new(timeframe: Timeframe) -> Self {
        Self(timeframe)
    }
}

#[async_trait]
impl Job for HotTagsCacheJob {
    fn name(&self) -> &'static str {
        match self.0 {
            Timeframe::Today => "hot-tags-cache-today",
            Timeframe::ThisWeek => "hot-tags-cache-this-week",
            Timeframe::ThisMonth => "hot-tags-cache-this-month",
            Timeframe::AllTime => "hot-tags-cache-all-time",
        }
    }

    async fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        match timeout(REFRESH_TIMEOUT, HotTags::fetch_and_cache(&self.0)).await {
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
            Arc::new(HotTagsCacheJob::new(Timeframe::Today)),
            Arc::new(HotTagsCacheJob::new(Timeframe::ThisWeek)),
            Arc::new(HotTagsCacheJob::new(Timeframe::ThisMonth)),
            Arc::new(HotTagsCacheJob::new(Timeframe::AllTime)),
        ]);

        let mut names = registry.job_names();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "hot-tags-cache-all-time",
                "hot-tags-cache-this-month",
                "hot-tags-cache-this-week",
                "hot-tags-cache-today",
            ]
        );
    }
}
