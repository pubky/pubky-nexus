//! Concrete jobs the daemon can register. The engine does not depend on these types.

mod hot_tags;
mod resource_changes_seed;
use resource_changes_seed::ResourceChangesSeedJob;
mod influencers;
mod most_followed;
mod trust_recompute;

pub use hot_tags::HotTagsCacheJob;
pub use influencers::InfluencersCacheJob;
pub use most_followed::MostFollowedCacheJob;
pub use trust_recompute::TrustRecomputeJob;

use std::sync::Arc;

use nexus_common::{types::Timeframe, TrustRankConfig};

use super::{JobRegistry, LOCK_TTL_SECS};

impl JobRegistry {
    /// This daemon's jobs: one cache-refresh job per timeframe, the most-followed ranking
    /// refresh, trust-recompute and resource-changes-seed. `trust_rank` configures only
    /// trust-recompute.
    pub fn catalog(trust_rank: &TrustRankConfig) -> Self {
        Self::new(vec![
            Arc::new(InfluencersCacheJob::new(Timeframe::Today)),
            Arc::new(InfluencersCacheJob::new(Timeframe::ThisWeek)),
            Arc::new(InfluencersCacheJob::new(Timeframe::ThisMonth)),
            Arc::new(InfluencersCacheJob::new(Timeframe::AllTime)),
            Arc::new(HotTagsCacheJob::new(Timeframe::Today)),
            Arc::new(HotTagsCacheJob::new(Timeframe::ThisWeek)),
            Arc::new(HotTagsCacheJob::new(Timeframe::ThisMonth)),
            Arc::new(HotTagsCacheJob::new(Timeframe::AllTime)),
            Arc::new(MostFollowedCacheJob),
            Arc::new(TrustRecomputeJob::build(trust_rank, LOCK_TTL_SECS)),
            Arc::new(ResourceChangesSeedJob),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::JobRegistry;
    use crate::config::DaemonConfig;
    use nexus_common::TrustRankConfig;

    #[test]
    fn catalog_names_are_the_eleven_jobs() {
        let mut names = JobRegistry::catalog(&TrustRankConfig::default()).job_names();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "hot-tags-cache-all-time",
                "hot-tags-cache-this-month",
                "hot-tags-cache-this-week",
                "hot-tags-cache-today",
                "influencers-cache-all-time",
                "influencers-cache-this-month",
                "influencers-cache-this-week",
                "influencers-cache-today",
                "most-followed-cache",
                "resource-changes-seed",
                "trust-recompute",
            ]
        );
    }

    #[tokio::test]
    async fn shipped_default_config_resolves_against_the_catalog() {
        let dir = tempfile::TempDir::new().unwrap();
        let config = DaemonConfig::read_or_create_config_file(dir.path().to_path_buf())
            .await
            .unwrap();
        JobRegistry::catalog(&config.trust_rank)
            .scheduled_jobs(&config)
            .expect("shipped [jobs.*] keys must match the catalog");
    }
}
