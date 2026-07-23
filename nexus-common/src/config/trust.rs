use pubky_app_specs::PubkyId;
use serde::{de::Error, Deserialize, Deserializer, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

use super::file::{default_config_dir_path, validate_and_expand_path};

/// Default teleport weight (`alpha` in `trust = alpha*v + (1-alpha)*M*trust`).
/// GDS's `dampingFactor` is `1 - alpha`.
pub const DEFAULT_TRUST_ALPHA: f64 = 0.35;
/// Default cap on PageRank power-iteration rounds.
pub const DEFAULT_TRUST_MAX_ITERATIONS: u32 = 200;
/// Default convergence tolerance: run close to full convergence rather than early-stop.
pub const DEFAULT_TRUST_TOLERANCE: f64 = 0.0000001;
/// Default max rows in a recompute CSV report.
pub const DEFAULT_TRUST_REPORT_LIMIT: usize = 10_000;

/// Dedupes the seed list, keeping first-occurrence order: duplicate ids would
/// otherwise inflate `sourceNodes` and skew the configured-vs-matched seed check.
fn deserialize_seed<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<PubkyId>, D::Error> {
    let seeds = Vec::<PubkyId>::deserialize(d)?;
    let mut seen = HashSet::new();
    Ok(seeds
        .into_iter()
        .filter(|id| seen.insert(id.clone()))
        .collect())
}

fn deserialize_alpha<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    let alpha = f64::deserialize(d)?;
    if !alpha.is_finite() || alpha <= 0.0 || alpha > 1.0 {
        return Err(D::Error::custom(format!(
            "trust.alpha ({alpha}) must be in the range (0, 1]"
        )));
    }
    Ok(alpha)
}

fn deserialize_max_iterations<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let max_iterations = u32::deserialize(d)?;
    if max_iterations == 0 {
        return Err(D::Error::custom("trust.max_iterations must be at least 1"));
    }
    Ok(max_iterations)
}

fn deserialize_report_limit<'de, D: Deserializer<'de>>(d: D) -> Result<usize, D::Error> {
    let report_limit = usize::deserialize(d)?;
    if report_limit == 0 {
        return Err(D::Error::custom("trust.report_limit must be at least 1"));
    }
    Ok(report_limit)
}

fn deserialize_tolerance<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    let tolerance = f64::deserialize(d)?;
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(D::Error::custom(format!(
            "trust.tolerance ({tolerance}) must be a finite, non-negative number"
        )));
    }
    Ok(tolerance)
}

fn deserialize_report_dir<'de, D: Deserializer<'de>>(d: D) -> Result<PathBuf, D::Error> {
    let path = PathBuf::deserialize(d)?;
    validate_and_expand_path(path).map_err(D::Error::custom)
}

/// Default directory for scheduled-run CSV reports.
pub fn default_trust_report_dir() -> PathBuf {
    default_config_dir_path().join("trust-reports")
}

/// Parameters for the seeded (personalized) PageRank trust computation, run via
/// the Neo4j GDS `pageRank` procedure over the follow graph. Covers *what* the
/// computation does; *when* it runs is a job concern (see
/// `[jobs.trust-recompute]` / [`super::JobConfig`]).
///
/// Seeds are weighted equally (`v` uniform, `1/N` over `seed`): GDS 2.13 (the
/// only version on Neo4j 5.26) has no weighted `sourceNodes`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct TrustRankConfig {
    /// Users trust originates from, each weighted equally (`v = 1/N`). Empty
    /// by default: an operator must configure a seed set before trust rank
    /// can be computed. Duplicate ids are silently deduplicated, keeping
    /// first-occurrence order.
    #[serde(deserialize_with = "deserialize_seed")]
    pub seed: Vec<PubkyId>,
    /// Teleport weight `alpha` in `trust = alpha*v + (1-alpha)*M*trust`.
    /// GDS's `dampingFactor` is derived as `1 - alpha`. Must be in (0, 1].
    #[serde(deserialize_with = "deserialize_alpha")]
    pub alpha: f64,
    /// Cap on power-iteration rounds. Must be at least 1.
    #[serde(deserialize_with = "deserialize_max_iterations")]
    pub max_iterations: u32,
    /// Convergence tolerance passed to GDS; kept low so the computation runs to
    /// convergence rather than stopping early. Must be finite and non-negative.
    #[serde(deserialize_with = "deserialize_tolerance")]
    pub tolerance: f64,
    /// When true, each recompute also writes a CSV report of the run to
    /// `report_dir` (default: false).
    pub report_enabled: bool,
    /// Directory recompute reports are written to, created if missing.
    /// Files are named `trust-report-<UTC timestamp>.csv`.
    #[serde(deserialize_with = "deserialize_report_dir")]
    pub report_dir: PathBuf,
    /// Max rows in a recompute report (top users by score). Must be ≥ 1.
    #[serde(deserialize_with = "deserialize_report_limit")]
    pub report_limit: usize,
    /// Hard cap on the GDS projection size (bytes). Default: None (no cap).
    pub max_projection_bytes: Option<u64>,
}

impl Default for TrustRankConfig {
    fn default() -> Self {
        Self {
            seed: Vec::new(),
            alpha: DEFAULT_TRUST_ALPHA,
            max_iterations: DEFAULT_TRUST_MAX_ITERATIONS,
            tolerance: DEFAULT_TRUST_TOLERANCE,
            report_enabled: false,
            report_dir: default_trust_report_dir(),
            report_limit: DEFAULT_TRUST_REPORT_LIMIT,
            max_projection_bytes: None,
        }
    }
}
