use async_trait::async_trait;
use nexus_common::types::DynError;
use nexus_common::TrustRankConfig;

/// Inputs for a seeded PageRank computation, decoupling [`TrustRankEngine`]
/// from [`TrustRankConfig`] so implementations ignore config fields they don't
/// use (e.g. `report_dir`).
#[derive(Debug, Clone)]
pub struct TrustRankParams {
    /// Users trust originates from, each weighted equally (`v = 1/N`).
    pub seed_ids: Vec<String>,
    /// Teleport weight `alpha` in `trust = alpha*v + (1-alpha)*M*trust`.
    pub alpha: f64,
    /// Cap on power-iteration rounds.
    pub max_iterations: u32,
    /// Convergence tolerance.
    pub tolerance: f64,
    /// Optional hard cap on the GDS projection size (bytes).
    pub max_projection_bytes: Option<u64>,
}

impl From<&TrustRankConfig> for TrustRankParams {
    fn from(config: &TrustRankConfig) -> Self {
        Self {
            seed_ids: config.seed.iter().map(|id| id.to_string()).collect(),
            alpha: config.alpha,
            max_iterations: config.max_iterations,
            tolerance: config.tolerance,
            max_projection_bytes: config.max_projection_bytes,
        }
    }
}

/// Summary of a completed trust-rank run.
#[derive(Debug, Clone)]
pub struct TrustRankStats {
    /// Number of users a score was written for.
    pub users_written: u64,
    /// Power-iteration rounds actually run.
    pub ran_iterations: u64,
    /// Whether the computation converged within `max_iterations`.
    pub did_converge: bool,
}

/// Computes seeded (personalized) PageRank trust, persisting each user's
/// score as a side effect and returning only run stats.
#[async_trait]
pub trait TrustRankEngine: Send + Sync {
    /// Runs the computation for `params` and returns run stats. Input
    /// validation (seed set, empty graph) is the implementation's job.
    async fn compute(&self, params: &TrustRankParams) -> Result<TrustRankStats, DynError>;
}
