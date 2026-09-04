use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use nexus_common::models::user::SocialGraphStatus;
use nexus_common::types::DynError;
use nexus_common::TrustRankConfig;
use opentelemetry::global;
use opentelemetry::metrics::{Counter, Meter};
use tracing::{debug, error, info, warn};

use super::engine::{TrustRankEngine, TrustRankParams};
use super::export::{read_scores, write_timestamped_csv};
use super::neo4j::GdsNeo4j;
use crate::jobs::Job;

/// OpenTelemetry meter name for all trust-rank metrics.
const METER_NAME: &str = "nexus.trust";

/// Publishes a finished ranking to whatever serves it. Injected into the job
/// for the same reason the engine is: so the unit tests can drive `run` without
/// a database behind it.
#[async_trait]
pub(crate) trait TrustProjection: Send + Sync {
    /// Republishes the read model from the scores just written to the graph.
    async fn publish(&self) -> Result<(), DynError>;
}

/// Rebuilds the Redis ranking that backs the social graph badge.
pub(crate) struct SocialGraphProjection;

#[async_trait]
impl TrustProjection for SocialGraphProjection {
    async fn publish(&self) -> Result<(), DynError> {
        SocialGraphStatus::reindex().await.map_err(Into::into)
    }
}

/// The trust-rank recompute as a runnable [`Job`]: runs the seeded PageRank
/// computation and, when a report dir is set, writes a CSV report of the run.
/// Build from resolved inputs with [`TrustRecomputeJob::new`] or from config
/// with [`TrustRecomputeJob::build`]; the job runner resolves its schedule from
/// the `[jobs.trust-recompute]` cron.
pub struct TrustRecomputeJob {
    params: TrustRankParams,
    engine: Box<dyn TrustRankEngine>,
    projection: Box<dyn TrustProjection>,
    report_dir: Option<PathBuf>,
    report_limit: usize,
    /// Runs that exhausted `max_iterations` without converging, so the written
    /// scores are the last (unconverged) iterate. A sustained nonzero rate means
    /// `max_iterations` or `tolerance` needs raising.
    max_iterations_reached: Counter<u64>,
}

impl TrustRecomputeJob {
    /// Builds the job from inputs. `report_dir` is `Some` to write a CSV, `None` to skip;
    /// `report_limit` caps rows in the report. Instruments come from the global
    /// meter (no-ops until an `SdkMeterProvider` is installed).
    pub(crate) fn new(
        params: TrustRankParams,
        engine: Box<dyn TrustRankEngine>,
        projection: Box<dyn TrustProjection>,
        report_dir: Option<PathBuf>,
        report_limit: usize,
    ) -> Self {
        Self::with_meter(
            params,
            engine,
            projection,
            report_dir,
            report_limit,
            global::meter(METER_NAME),
        )
    }

    /// [`new`](Self::new) with an explicit `meter`, so tests can install a local
    /// `SdkMeterProvider` and read the counter back.
    pub(crate) fn with_meter(
        params: TrustRankParams,
        engine: Box<dyn TrustRankEngine>,
        projection: Box<dyn TrustProjection>,
        report_dir: Option<PathBuf>,
        report_limit: usize,
        meter: Meter,
    ) -> Self {
        let max_iterations_reached = meter
            .u64_counter("trust.recompute.max_iterations_reached")
            .with_description("Trust rank runs that hit max_iterations without converging")
            .build();
        Self {
            params,
            engine,
            projection,
            report_dir,
            report_limit,
            max_iterations_reached,
        }
    }

    /// Builds the job from its trust-rank config and the run lease TTL.
    pub fn build(config: &TrustRankConfig, lock_ttl_secs: u64) -> Self {
        // Sweep age = 2× the lease TTL (which already includes LEASE_MARGIN), so
        // it sits well past lease expiry and the sweep can never race a live run.
        Self::new(
            TrustRankParams::from(config),
            Box::new(GdsNeo4j::new(true, Duration::from_secs(2 * lock_ttl_secs))),
            Box::new(SocialGraphProjection),
            config.report_enabled.then(|| config.report_dir.clone()),
            config.report_limit,
        )
    }
}

#[async_trait]
impl Job for TrustRecomputeJob {
    fn name(&self) -> &'static str {
        "trust-recompute"
    }

    async fn run(&self) -> Result<(), DynError> {
        let stats = self.engine.compute(&self.params).await?;
        debug!(
            users_written = stats.users_written,
            ran_iterations = stats.ran_iterations,
            did_converge = stats.did_converge,
            "Trust rank run stats"
        );
        if !stats.did_converge {
            self.max_iterations_reached.add(1, &[]);
            warn!(
                max_iterations = self.params.max_iterations,
                tolerance = self.params.tolerance,
                "Trust rank hit max_iterations without converging"
            );
        }

        // Report failures are logged, not fatal: scores are already persisted.
        if let Some(dir) = &self.report_dir {
            match read_scores(self.report_limit).await {
                Ok(scores) => match write_timestamped_csv(dir, &scores).await {
                    Ok(path) => info!(path = %path.display(), "Trust rank report written"),
                    Err(e) => error!("Failed to write trust rank report: {e:?}"),
                },
                Err(e) => error!("Failed to read trust scores for report: {e:?}"),
            }
        }

        // Last, so a Redis blip cannot cost the report of a compute that already
        // succeeded. Still fatal: fresh scores nobody can read leave the badge on
        // yesterday's ranking, and that should not pass as a clean run.
        self.projection.publish().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    use opentelemetry::metrics::MeterProvider;
    use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData, ResourceMetrics};
    use opentelemetry_sdk::metrics::{InMemoryMetricExporter, SdkMeterProvider};

    use super::super::engine::{TrustRankEngine, TrustRankParams, TrustRankStats};
    use super::*;

    // The real projection needs Neo4j and Redis; these tests drive `run` with
    // no infrastructure at all.
    struct NoOpProjection;

    #[async_trait]
    impl TrustProjection for NoOpProjection {
        async fn publish(&self) -> Result<(), DynError> {
            Ok(())
        }
    }

    // Dumb stub: bumps a shared counter and replays a canned result. The counter
    // is shared so the test can inspect it after the engine moves into the job.
    struct MockEngine {
        calls: Arc<AtomicU32>,
        fail: bool,
        did_converge: bool,
    }

    impl MockEngine {
        fn new(calls: &Arc<AtomicU32>, fail: bool) -> Self {
            Self {
                calls: Arc::clone(calls),
                fail,
                did_converge: true,
            }
        }
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
                did_converge: self.did_converge,
            })
        }
    }

    /// A job whose counters feed an in-memory exporter. Call
    /// `provider.force_flush()` before reading `exporter.get_finished_metrics()`.
    fn metered_job(
        engine: Box<dyn TrustRankEngine>,
    ) -> (TrustRecomputeJob, SdkMeterProvider, InMemoryMetricExporter) {
        let exporter = InMemoryMetricExporter::default();
        let provider = SdkMeterProvider::builder()
            .with_periodic_exporter(exporter.clone())
            .build();
        let job = TrustRecomputeJob::with_meter(
            params(),
            engine,
            Box::new(NoOpProjection),
            None,
            10,
            provider.meter("test"),
        );
        (job, provider, exporter)
    }

    /// Sum of a `u64` counter's data points across all exported metrics.
    fn counter_value(metrics: &[ResourceMetrics], name: &str) -> u64 {
        let mut total = 0;
        for rm in metrics {
            for sm in rm.scope_metrics() {
                for m in sm.metrics().filter(|m| m.name() == name) {
                    let AggregatedMetrics::U64(MetricData::Sum(sum)) = m.data() else {
                        continue;
                    };
                    total += sum.data_points().map(|dp| dp.value()).sum::<u64>();
                }
            }
        }
        total
    }

    fn params() -> TrustRankParams {
        TrustRankParams {
            seed_ids: vec!["seed".to_string()],
            alpha: 0.85,
            max_iterations: 20,
            tolerance: 1e-6,
            max_projection_bytes: None,
        }
    }

    // No report_dir: run() computes and returns without touching the store.
    #[tokio::test]
    async fn run_without_report_computes_and_skips_report() {
        let calls = Arc::new(AtomicU32::new(0));
        let engine = MockEngine::new(&calls, false);
        let job = TrustRecomputeJob::new(
            params(),
            Box::new(engine),
            Box::new(NoOpProjection),
            None,
            10,
        );

        job.run().await.expect("run should succeed");

        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    // A compute error propagates and short-circuits before the report block,
    // so run() fails without ever reaching the store (report_dir is Some here).
    #[tokio::test]
    async fn run_propagates_compute_error_before_report() {
        let calls = Arc::new(AtomicU32::new(0));
        let engine = MockEngine::new(&calls, true);
        let job = TrustRecomputeJob::new(
            params(),
            Box::new(engine),
            Box::new(NoOpProjection),
            Some(PathBuf::from("/does-not-matter")),
            10,
        );

        let err = job.run().await.expect_err("run should fail");

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(err.to_string(), "compute failed");
    }

    // A run that exhausted max_iterations bumps the counter.
    #[tokio::test]
    async fn run_counts_max_iterations_reached_when_not_converged() {
        let calls = Arc::new(AtomicU32::new(0));
        let engine = MockEngine {
            did_converge: false,
            ..MockEngine::new(&calls, false)
        };
        let (job, provider, exporter) = metered_job(Box::new(engine));

        job.run().await.expect("run should succeed");
        provider.force_flush().expect("flush should succeed");

        let metrics = exporter
            .get_finished_metrics()
            .expect("metrics should be exported");
        assert_eq!(
            counter_value(&metrics, "trust.recompute.max_iterations_reached"),
            1
        );
    }

    // A converged run leaves the counter untouched.
    #[tokio::test]
    async fn run_does_not_count_max_iterations_reached_when_converged() {
        let calls = Arc::new(AtomicU32::new(0));
        let (job, provider, exporter) = metered_job(Box::new(MockEngine::new(&calls, false)));

        job.run().await.expect("run should succeed");
        provider.force_flush().expect("flush should succeed");

        let metrics = exporter
            .get_finished_metrics()
            .expect("metrics should be exported");
        assert_eq!(
            counter_value(&metrics, "trust.recompute.max_iterations_reached"),
            0
        );
    }
}
