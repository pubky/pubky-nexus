use crate::events::processor::EventProcessorFactory;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::watch::Receiver;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};
use super::RollingWindow;
use super::ProcessResult;


/// Manages the periodic processing of homeservers in isolated cycles
pub struct CycleProcessor {
    event_processor_factory: Arc<EventProcessorFactory>,
    period: Duration,
    cycle: u64,
    timeout: Duration,
    shutdown_rx: Receiver<bool>,
}

impl CycleProcessor {
    pub fn new(
        event_processor_factory: Arc<EventProcessorFactory>,
        shutdown_rx: Receiver<bool>,
        period: Duration,
    ) -> Self {
        Self {
            event_processor_factory,
            period,
            cycle: 0,
            timeout: Duration::from_secs(super::PROCESSING_TIMEOUT_SECS),
            shutdown_rx,
        }
    }

    pub fn cycle_count(&self) -> u64 {
        self.cycle
    }

    /// Executes one full processing cycle
    pub async fn run_cycle(&mut self) -> Result<Duration, DynError> {
        self.cycle += 1;
        let cycle_start = Instant::now();

        info!(cycle = self.cycle, "Starting cycle");

        let homeservers: Vec<String> = Homeserver::get_all_from_graph().await?;

        if homeservers.is_empty() {
            warn!("No homeservers found... skipping cycle");
            return Ok(Duration::from_millis(0));
        }

        info!(
            cycle = self.cycle,
            count = homeservers.len(),
            "Processing homeservers"
        );

        // Process with rolling window
        let (ok, fail) = self.exec_watcher(homeservers).await;

        let elapsed = cycle_start.elapsed();
        info!(
            cycle = self.cycle,
            ok,
            fail,
            elapsed_ms = elapsed.as_millis(),
            "Cycle finished"
        );

        Ok(elapsed)
    }

    /// Processes a list of homeservers with bounded concurrency
    ///
    /// # Arguments
    /// - `homeservers`: The list of homeservers to process
    /// # Returns
    /// - `(ok, fail)`: The number of successful and failed homeservers
    pub async fn exec_watcher(&self, homeservers: Vec<String>) -> (usize, usize) {
        let mut window = RollingWindow::new(super::MAX_CONCURRENT);
        let mut iter = homeservers.into_iter();

        // Statistics
        let mut ok = 0usize;
        let mut fail = 0usize;

        // Prime the window
        window.fill(
            &mut iter,
            self.event_processor_factory.clone(),
            self.timeout,
            self.shutdown_rx.clone(),
        );

        // Process until all complete
        while let Some(result) = window.join_next().await {
            match result {
                ProcessResult::Success => ok += 1,
                ProcessResult::Error(e) => {
                    fail += 1;
                    error!(error = %e, "Worker failed");
                }
                ProcessResult::Panic(e) => {
                    fail += 1;
                    error!(%e, "Worker panicked");
                }
            }

            // Refill to maintain window size
            window.fill(
                &mut iter,
                self.event_processor_factory.clone(),
                self.timeout,
                self.shutdown_rx.clone(),
            );
        }

        (ok, fail)
    }

    /// Waits before starting the next cycle to maintain the target period
    /// # Arguments
    /// - `elapsed`: The elapsed time since the last cycle
    /// - `shutdown_rx`: The receiver to listen for shutdown signals
    /// # Returns
    /// - `true` if the cycle should continue, `false` if the cycle should stop based on the timeout
    pub async fn pace_cycle(&self, elapsed: Duration, shutdown_rx: &mut Receiver<bool>) -> bool {
        if elapsed < self.period {
            let remaining = self.period - elapsed;
            info!(
                cycle = self.cycle,
                remaining_ms = remaining.as_millis(),
                "Pacing before next cycle"
            );

            tokio::select! {
                _ = sleep(remaining) => true,
                _ = shutdown_rx.changed() => false,
            }
        } else {
            warn!(
                cycle = self.cycle,
                over_ms = (elapsed - self.period).as_millis(),
                "Cycle exceeded target period"
            );
            true
        }
    }
}