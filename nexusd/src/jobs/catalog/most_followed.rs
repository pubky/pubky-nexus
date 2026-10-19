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

#[derive(Debug, thiserror::Error)]
#[error("most-followed ranking refresh timed out after {after:?}")]
struct RefreshTimedOut {
    after: Duration,
}

/// Rebuild the whole `Sorted:Users:MostFollowed` ranking from the graph's follower
/// counts on a schedule. That ranking has no TTL and no other writer, so this job is
/// the one that keeps it current.
pub struct MostFollowedCacheJob;

#[async_trait]
impl Job for MostFollowedCacheJob {
    fn name(&self) -> &'static str {
        "most-followed-cache"
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
