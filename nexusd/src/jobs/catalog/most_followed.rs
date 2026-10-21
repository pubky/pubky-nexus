use async_trait::async_trait;
use nexus_common::models::user::UserStream;
use std::error::Error;
use std::time::Duration;
use tokio::time::timeout;

use crate::jobs::Job;

/// Cap for one graph scan + swap. Must outlast the API request timeout (a miss cannot
/// finish a scan this long) and stay under the runner's MAX_RUN. `tokio::time::timeout`
/// on purpose: this is an I/O window, not the wall-clock lease.
const REFRESH_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Once a day at 04:06 UTC, clear of the other jobs' times. The ranking has no TTL
/// and no other writer, so the job runs on this unless the config says otherwise.
const MOST_FOLLOWED_CRON: &str = "0 6 4 * * *";

#[derive(Debug, thiserror::Error)]
#[error("most-followed ranking refresh timed out after {after:?}")]
struct RefreshTimedOut {
    after: Duration,
}

/// Rebuild the whole `Sorted:Users:MostFollowed` ranking from the graph's follower
/// counts on a schedule. That ranking has no TTL and no other writer, so this job is
/// the one that keeps it current, on [`MOST_FOLLOWED_CRON`] by default.
pub struct MostFollowedCacheJob;

#[async_trait]
impl Job for MostFollowedCacheJob {
    fn name(&self) -> &'static str {
        "most-followed-cache"
    }

    fn default_cron(&self) -> Option<&'static str> {
        Some(MOST_FOLLOWED_CRON)
    }

    async fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        match timeout(REFRESH_TIMEOUT, UserStream::refresh_most_followed()).await {
            Ok(result) => Ok(result?),
            Err(_elapsed) => Err(Box::new(RefreshTimedOut {
                after: REFRESH_TIMEOUT,
            })),
        }
    }
}
