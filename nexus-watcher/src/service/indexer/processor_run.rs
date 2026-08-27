//! Types that control and describe a single event processor run.

use std::time::Duration;

use tokio::sync::watch;

use crate::errors::EventProcessorError;

/// Successful outcome of one event processor invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunCompletion {
    /// The processor finished normally.
    Completed,
    /// The processor stopped cooperatively after exhausting its run budget.
    BudgetExhausted,
}

/// Timeout behavior for an event processor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutPolicy {
    HardAbort,
    Cooperative { grace: Duration },
}

/// State scoped to one invocation of an event processor.
#[derive(Clone)]
pub struct RunContext {
    budget_exhaustion_rx: watch::Receiver<bool>,
}

impl RunContext {
    /// Creates a run context and the sender used to signal budget exhaustion.
    pub fn with_budget_signal() -> (watch::Sender<bool>, Self) {
        let (budget_exhaustion_tx, budget_exhaustion_rx) = watch::channel(false);
        (
            budget_exhaustion_tx,
            Self {
                budget_exhaustion_rx,
            },
        )
    }

    /// Creates a context that never receives a budget-expiry signal.
    pub fn inactive() -> Self {
        Self::with_budget_signal().1
    }

    /// Returns whether the run budget expired and requested a cooperative stop.
    ///
    /// This check is non-blocking and should be called at safe processing boundaries.
    pub fn is_budget_exhausted(&self) -> bool {
        *self.budget_exhaustion_rx.borrow()
    }
}

pub type ProcessorResult = Result<RunCompletion, EventProcessorError>;

/// Possible error types of an event processor run.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("Internal error: {0}")]
    Internal(EventProcessorError),
    #[error("Execution panicked")]
    Panicked,
    #[error("Execution timed out")]
    TimedOut,
}

impl RunError {
    pub fn is_panic(&self) -> bool {
        matches!(self, RunError::Panicked)
    }

    pub fn is_timeout(&self) -> bool {
        matches!(self, RunError::TimedOut)
    }
}
