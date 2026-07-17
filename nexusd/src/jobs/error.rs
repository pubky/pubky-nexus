use nexus_common::types::DynError;
use thiserror::Error;

/// A run lock backend failure.
#[derive(Debug, Error)]
#[error("run lock backend failed: {0}")]
pub struct LockError(#[source] pub DynError);

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
