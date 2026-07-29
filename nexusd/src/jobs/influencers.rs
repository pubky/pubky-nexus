use async_trait::async_trait;
use nexus_common::models::error::ModelError;
use nexus_common::models::user::Influencers;
use std::error::Error;
use std::time::Duration;
use tokio::time::timeout;

use super::Job;

/// Per-timeframe ceiling for one graph scan + cache write. Sized so all three
/// timeframes together stay well inside the runner's MAX_RUN deadline.
/// Monotonic on purpose: `tokio::time::timeout`, not `sleep_wall`. The
/// wall-clock discipline elsewhere exists so a run can't outlive its lease;
/// a 60s I/O window is unaffected by host suspend.
const REFRESH_TIMEOUT: Duration = Duration::from_secs(60);

/// Refresh the global influencer cache on a schedule.
pub struct InfluencersCacheJob;

#[async_trait]
impl Job for InfluencersCacheJob {
    fn name(&self) -> &'static str {
        "influencers_cache"
    }

    async fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        Influencers::refresh_global_cache_with(&|tf| async move {
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
