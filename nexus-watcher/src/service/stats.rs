use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessorRunStatus {
    FailedToBuild,
    Ok,
    Error,
    Panic,
    Timeout,
}

pub struct ProcessorRunStats {
    pub hs_id: String,
    pub duration: Duration,
    pub status: ProcessorRunStatus,
}

#[derive(Default)]
pub struct RunAllProcessorsStats {
    pub stats: Vec<ProcessorRunStats>,
}

impl RunAllProcessorsStats {
    pub(crate) fn add_run_result(
        &mut self,
        hs_id: String,
        duration: Duration,
        status: ProcessorRunStatus,
    ) {
        let individual_run_stats = ProcessorRunStats {
            hs_id,
            duration,
            status,
        };
        self.stats.push(individual_run_stats);
    }

    fn count(&self, status: ProcessorRunStatus) -> usize {
        self.stats.iter().filter(|ps| ps.status == status).count()
    }

    /// Number of homeservers where processing were successful
    pub fn count_ok(&self) -> usize {
        self.count(ProcessorRunStatus::Ok)
    }

    /// Number of homeservers where processing failed with Err
    pub fn count_error(&self) -> usize {
        self.count(ProcessorRunStatus::Error)
    }

    /// Number of homeservers where processing panicked
    pub fn count_panic(&self) -> usize {
        self.count(ProcessorRunStatus::Panic)
    }

    /// Number of homeservers where processing timed out
    pub fn count_timeout(&self) -> usize {
        self.count(ProcessorRunStatus::Timeout)
    }

    /// Number of homeservers where processing failed to start
    pub fn count_failed_to_build(&self) -> usize {
        self.count(ProcessorRunStatus::FailedToBuild)
    }
}

/// Wrapper around `RunAllProcessorsStats` which indicates they've been processed
pub struct ProcessedStats(pub RunAllProcessorsStats);
