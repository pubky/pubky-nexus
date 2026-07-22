use nexus_common::types::DynError;
use thiserror::Error;

/// A run lock backend failure.
///
/// Type-erased so alternative lock backends keep their error types out of the trait.
#[derive(Debug, Error)]
#[error("run lock backend failed: {0}")]
pub struct LockError(#[source] pub Box<dyn std::error::Error + Send + Sync>);

pub type LockResult<T> = Result<T, LockError>;

#[derive(Debug, Error)]
pub enum JobError {
    #[error("job {job:?} is already running")]
    AlreadyRunning { job: &'static str },

    #[error("run lock error: {0}")]
    Lock(#[source] LockError),

    #[error("unknown job(s) {unknown:?}; available jobs: {available}")]
    UnknownJobConfig {
        unknown: Vec<String>,
        available: String,
    },

    #[error("unknown job {name:?}; available jobs: {available}")]
    UnknownJobName { name: String, available: String },

    #[error("[jobs.{job}]: {source}")]
    InvalidCron {
        job: String,
        #[source]
        source: CronParseError,
    },

    /// Boxed until StackManager::setup returns a typed error instead of DynError.
    #[error("stack setup failed: {0}")]
    Stack(#[source] DynError),

    #[error("job {job:?} exceeded its {after:?} run deadline and was abandoned")]
    TimedOut {
        job: &'static str,
        after: std::time::Duration,
    },

    /// Boxed: jobs are heterogeneous and the runner only logs the error.
    #[error("job {job:?} failed: {source}")]
    Run {
        job: &'static str,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

#[derive(Debug, Error)]
#[error("cron expression ({expr:?}): {reason}")]
pub struct CronParseError {
    pub expr: String,
    pub reason: String,
}
