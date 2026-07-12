use std::path::PathBuf;

use async_trait::async_trait;
use nexus_common::types::DynError;
use nexus_common::TrustRankConfig;
use tracing::{error, info};

use super::engine::{TrustRankEngine, TrustRankParams};
use super::export::{read_scores, write_timestamped_csv};
use super::neo4j::GdsNeo4j;
use crate::jobs::Job;

/// The trust-rank recompute as a runnable [`Job`]: runs the seeded PageRank
/// computation and, when a report dir is set, writes a CSV report of the run.
/// Build from resolved inputs with [`TrustRecomputeJob::new`] or from config
/// with [`TrustRecomputeJob::from_config`]; the job runner resolves its
/// schedule from the `[jobs.trust-recompute]` cron.
pub struct TrustRecomputeJob {
    params: TrustRankParams,
    engine: Box<dyn TrustRankEngine>,
    report_dir: Option<PathBuf>,
}

impl TrustRecomputeJob {
    /// Builds the job from already-resolved inputs. `report_dir` is `Some` to
    /// write a CSV report after each run, `None` to skip reporting.
    pub fn new(
        params: TrustRankParams,
        engine: Box<dyn TrustRankEngine>,
        report_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            params,
            engine,
            report_dir,
        }
    }

    /// Builds the job from its trust-rank config. The cron is resolved
    /// separately from the `[jobs.trust-recompute]` section.
    pub fn from_config(config: &TrustRankConfig) -> Self {
        Self::new(
            TrustRankParams::from(config),
            Box::new(GdsNeo4j),
            config.report_enabled.then(|| config.report_dir.clone()),
        )
    }
}

#[async_trait]
impl Job for TrustRecomputeJob {
    fn name(&self) -> &'static str {
        "trust-recompute"
    }

    async fn run(&self) -> Result<(), DynError> {
        self.engine.compute(&self.params).await?;

        // Report failures are logged, not fatal: scores are already persisted.
        if let Some(dir) = &self.report_dir {
            match read_scores().await {
                Ok(scores) => match write_timestamped_csv(dir, &scores).await {
                    Ok(path) => info!(path = %path.display(), "Trust rank report written"),
                    Err(e) => error!("Failed to write trust rank report: {e:?}"),
                },
                Err(e) => error!("Failed to read trust scores for report: {e:?}"),
            }
        }
        Ok(())
    }
}
