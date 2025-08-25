use crate::events::processor::EventProcessorFactory;
use nexus_common::types::DynError;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::watch::Receiver;
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio::time::Duration;
use tracing::{error, info};
use super::ProcessResult;


/// Manages a rolling window of concurrent tasks
pub struct RollingWindow {
    set: JoinSet<Result<(), DynError>>,
    max_size: usize,
}

impl RollingWindow {
    pub fn new(max_size: usize) -> Self {
        Self {
            set: JoinSet::new(),
            max_size,
        }
    }

    /// Checks if the window is full
    pub fn is_full(&self) -> bool {
        self.set.len() >= self.max_size
    }

    /// Spawns new tasks into the rolling window until it is full
    pub fn fill(
        &mut self,
        iter: &mut impl Iterator<Item = String>,
        event_processor_factory: Arc<EventProcessorFactory>,
        timeout: Duration,
        shutdown_rx: Receiver<bool>,
    ) {
        // loop until the window is full or there are no more homeservers to process
        while !self.is_full() {
            match iter.next() {
                Some(homeserver_id) => {
                    // spawn a task to process the homeserver
                    self.set.spawn(process_homeserver(
                        homeserver_id,
                        event_processor_factory.clone(),
                        timeout,
                        shutdown_rx.clone(),
                    ));
                }
                None => break,
            }
        }
    }

    /// Waits for the next task in the window to complete
    pub async fn join_next(&mut self) -> Option<ProcessResult> {
        match self.set.join_next().await? {
            Ok(Ok(())) => Some(ProcessResult::Success),
            Ok(Err(e)) => Some(ProcessResult::Error(e)),
            Err(e) => {
                if e.is_panic() {
                    Some(ProcessResult::Panic(e))
                } else {
                    Some(ProcessResult::Error(e.into()))
                }
            }
        }
    }
}

/// Runs a single homeserver event processing with timeout
/// # Arguments
/// - `homeserver_id`: The ID of the homeserver to process
/// - `event_processor_factory`: The factory to create the event processor
/// - `task_timeout`: The timeout for the task
/// - `shutdown_rx`: The receiver to listen for shutdown signals
async fn process_homeserver(
    homeserver_id: String,
    event_processor_factory: Arc<EventProcessorFactory>,
    task_timeout: Duration,
    shutdown_rx: Receiver<bool>,
) -> Result<(), DynError> {
    let start = Instant::now();

    // Initialize event processor
    let event_processor = event_processor_factory.build(&homeserver_id).await?;

    // Process with timeout
    match timeout(task_timeout, event_processor.run(shutdown_rx)).await {
        Ok(Ok(cursor)) => {
            info!(
                homeserver = %homeserver_id,
                next_cursor = cursor,
                took_ms = start.elapsed().as_millis(),
                "Processed"
            );
            Ok(())
        }
        Ok(Err(e)) => {
            error!(homeserver = %homeserver_id, error = %e, "Processing error");
            Err(e)
        }
        Err(_) => {
            error!(
                homeserver = %homeserver_id,
                timeout_secs = task_timeout.as_secs(),
                "Processing timeout"
            );
            Err("processing timeout".into())
        }
    }
}
