use std::path::PathBuf;
use std::time::Duration;

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
/// with [`TrustRecomputeJob::build`]; the job runner resolves its schedule from
/// the `[jobs.trust-recompute]` cron.
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

    /// Builds the job from its trust-rank config and the run lease TTL.
    pub fn build(config: &TrustRankConfig, lock_ttl_secs: u64) -> Self {
        // Sweep age = 2× the lease TTL (which already includes LEASE_MARGIN), so
        // it sits well past lease expiry and the sweep can never race a live run.
        Self::new(
            TrustRankParams::from(config),
            Box::new(GdsNeo4j::new(true, Duration::from_secs(2 * lock_ttl_secs))),
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    use super::super::engine::{TrustRankEngine, TrustRankParams, TrustRankStats};
    use super::*;

    // Dumb stub: bumps a shared counter and replays a canned result. The counter
    // is shared so the test can inspect it after the engine moves into the job.
    struct MockEngine {
        calls: Arc<AtomicU32>,
        fail: bool,
    }

    #[async_trait]
    impl TrustRankEngine for MockEngine {
        async fn compute(&self, _params: &TrustRankParams) -> Result<TrustRankStats, DynError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                return Err("compute failed".into());
            }
            Ok(TrustRankStats {
                users_written: 1,
                ran_iterations: 1,
                did_converge: true,
            })
        }
    }

    fn params() -> TrustRankParams {
        TrustRankParams {
            seed_ids: vec!["seed".to_string()],
            alpha: 0.85,
            max_iterations: 20,
            tolerance: 1e-6,
        }
    }

    // No report_dir: run() computes and returns without touching the store.
    #[tokio::test]
    async fn run_without_report_computes_and_skips_report() {
        let calls = Arc::new(AtomicU32::new(0));
        let engine = MockEngine {
            calls: Arc::clone(&calls),
            fail: false,
        };
        let job = TrustRecomputeJob::new(params(), Box::new(engine), None);

        job.run().await.expect("run should succeed");

        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    // A compute error propagates and short-circuits before the report block,
    // so run() fails without ever reaching the store (report_dir is Some here).
    #[tokio::test]
    async fn run_propagates_compute_error_before_report() {
        let calls = Arc::new(AtomicU32::new(0));
        let engine = MockEngine {
            calls: Arc::clone(&calls),
            fail: true,
        };
        let job = TrustRecomputeJob::new(
            params(),
            Box::new(engine),
            Some(PathBuf::from("/does-not-matter")),
        );

        let err = job.run().await.expect_err("run should fail");

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(err.to_string(), "compute failed");
    }
}
