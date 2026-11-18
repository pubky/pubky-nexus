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
    /// refresh, trust-recompute, and resource-changes-seed with its variant that may publish an
    /// empty feed. `trust_rank` configures only trust-recompute.
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
            Arc::new(ResourceChangesSeedJob { allow_empty: false }),
            Arc::new(ResourceChangesSeedJob { allow_empty: true }),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::JobRegistry;
    use crate::config::DaemonConfig;
    use nexus_common::TrustRankConfig;

    #[test]
    fn catalog_names_are_the_twelve_jobs() {
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
                "resource-changes-seed-empty",
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

    /// A job that may not be scheduled has no schedule of its own either: with one, every
    /// startup would fail asking to remove a cron nobody configured.
    #[test]
    fn unschedulable_jobs_have_no_default_cron() {
        use std::sync::Arc;

        let registry = JobRegistry::catalog(&TrustRankConfig::default());
        let unschedulable: Vec<_> = registry
            .jobs()
            .iter()
            .filter(|job| !job.schedulable())
            .map(Arc::as_ref)
            .collect();
        assert_eq!(unschedulable.len(), 2, "the two seed jobs");
        for job in unschedulable {
            assert_eq!(job.default_cron(), None, "{}", job.name());
        }
    }

    /// Every seed run resets every consumer of the feed, so neither seed job may be given a
    /// cron: config resolution (`nexusd run`, and `nexusd jobs run` before it runs a job)
    /// fails naming the section. Without a cron, as shipped, both stay runnable on demand.
    #[tokio::test]
    async fn seed_jobs_reject_a_cron() {
        use crate::config::CONFIG_FILE_NAME;
        use crate::jobs::JobError;
        use nexus_common::file::ConfigLoader;

        let dir = tempfile::TempDir::new().unwrap();
        let shipped = DaemonConfig::read_or_create_config_file(dir.path().to_path_buf())
            .await
            .unwrap();
        let registry = JobRegistry::catalog(&shipped.trust_rank);
        registry.scheduled_jobs(&shipped).expect("shipped config");
        let toml = std::fs::read_to_string(dir.path().join(CONFIG_FILE_NAME)).unwrap();
        for job in ["resource-changes-seed", "resource-changes-seed-empty"] {
            // A section without a cron is still accepted.
            let config = DaemonConfig::try_from_str(&format!("{toml}\n[jobs.{job}]\n")).unwrap();
            registry
                .scheduled_jobs(&config)
                .expect("a section without cron");
            let config = DaemonConfig::try_from_str(&format!(
                "{toml}\n[jobs.{job}]\ncron = \"0 0 3 * * *\"\n"
            ))
            .unwrap();
            let err = registry.scheduled_jobs(&config).err().expect("rejected");
            assert!(
                matches!(err, JobError::UnschedulableJob { job: name } if name == job),
                "{job}: {err:?}"
            );
            let text = err.to_string();
            assert!(text.contains(&format!("[jobs.{job}]")), "{text}");
            assert!(text.contains("on demand"), "{text}");
        }
    }
}
