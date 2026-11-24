//! The generic event-processing toolkit: framing a homeserver's `/events/` body, parsing its
//! lines, running a processor and waiting for it, running processors across many homeservers,
//! and the hooks through which an application
//! plugs in its own handling and retry policy.
//!
//! Like [`homeserver`](crate::homeserver), it depends on no Nexus type: the processor is
//! generic over its event and error types, and retry is opt-in through [`RetryableError`] and
//! [`EventRetryScheduler`].

mod events;
mod processor;
mod runner;
mod traits;

pub use events::{EventBatch, CURSOR_PREFIX};
pub use processor::{
    dispatch_retryable_error, run_on_own_task, RunError, RunOutcome, TEventProcessor,
    SLOW_RUN_THRESHOLD,
};
pub use runner::{
    status_from_run_result, BuildError, ProcessedStats, ProcessorRunStats, ProcessorRunStatus,
    RunAllProcessorsStats, TEventProcessorRunner,
};
pub use traits::{EventRetryScheduler, LineParseOutcome, ParseFromLine, RetryableError};
