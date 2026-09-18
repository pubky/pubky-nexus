//! Concrete jobs the daemon can register. The engine does not depend on these types.

mod hot_tags;
mod influencers;
mod trust_recompute;

pub use hot_tags::HotTagsCacheJob;
pub use influencers::InfluencersCacheJob;
pub use trust_recompute::TrustRecomputeJob;

use std::sync::Arc;

use nexus_common::{
    types::{CacheTimeframe, Timeframe},
    TrustRankConfig,
};

use super::{JobRegistry, LOCK_TTL_SECS};

impl JobRegistry {
    /// This daemon's jobs: one cache-refresh job per timeframe, plus trust-recompute.
    /// `trust_rank` configures only the latter.
    pub fn catalog(trust_rank: &TrustRankConfig) -> Self {
        Self::new(vec![
            Arc::new(InfluencersCacheJob::new(CacheTimeframe::Today)),
            Arc::new(InfluencersCacheJob::new(CacheTimeframe::ThisWeek)),
            Arc::new(InfluencersCacheJob::new(CacheTimeframe::ThisMonth)),
            Arc::new(HotTagsCacheJob::new(Timeframe::Today)),
            Arc::new(HotTagsCacheJob::new(Timeframe::ThisWeek)),
            Arc::new(HotTagsCacheJob::new(Timeframe::ThisMonth)),
            Arc::new(HotTagsCacheJob::new(Timeframe::AllTime)),
            Arc::new(TrustRecomputeJob::build(trust_rank, LOCK_TTL_SECS)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::JobRegistry;
    use nexus_common::{DaemonConfig, TrustRankConfig};

    #[test]
    fn catalog_names_are_the_eight_jobs() {
        let mut names = JobRegistry::catalog(&TrustRankConfig::default()).job_names();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "hot-tags-cache-all-time",
                "hot-tags-cache-this-month",
                "hot-tags-cache-this-week",
                "hot-tags-cache-today",
                "influencers-cache-this-month",
                "influencers-cache-this-week",
                "influencers-cache-today",
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
