use crate::events::processor::EventProcessorFactory;
use crate::NexusWatcherBuilder;
use nexus_common::file::ConfigLoader;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use nexus_common::utils::create_shutdown_rx;
use nexus_common::{DaemonConfig, WatcherConfig};
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::watch::Receiver;
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

pub const WATCHER_CONFIG_FILE_NAME: &str = "watcher-config.toml";
// Max homeservers processed concurrently within a cycle
pub const MAX_CONCURRENT: usize = 3;
// Per-homeserver hard timeout (seconds)
pub const PROCESSING_TIMEOUT_SECS: u64 = 120;

pub struct NexusWatcher {}

impl NexusWatcher {
    /// Creates a new instance with default configuration
    pub fn builder() -> NexusWatcherBuilder {
        NexusWatcherBuilder::default()
    }

    /// Loads the [WatcherConfig] from [WATCHER_CONFIG_FILE_NAME] in the given path and starts the Nexus Watcher.
    ///
    /// If no [WatcherConfig] file is found, it defaults to [NexusWatcher::start_from_daemon].
    ///
    /// ### Arguments
    ///
    /// - `config_dir`: the directory where the config file is expected to be
    /// - `shutdown_rx`: optional shutdown signal. If none is provided, a default one will be created, listening for Ctrl-C.
    pub async fn start_from_path(
        config_dir: PathBuf,
        shutdown_rx: Option<Receiver<bool>>,
    ) -> Result<(), DynError> {
        let shutdown_rx = shutdown_rx.unwrap_or_else(create_shutdown_rx);

        match WatcherConfig::load(config_dir.join(WATCHER_CONFIG_FILE_NAME)).await {
            Ok(config) => NexusWatcherBuilder(config).start(Some(shutdown_rx)).await,
            Err(_) => NexusWatcher::start_from_daemon(config_dir, Some(shutdown_rx)).await,
        }
    }

    /// Derives the [WatcherConfig] from [DaemonConfig] (nexusd service config), loads it and starts the Watcher.
    ///
    /// If a [DaemonConfig] is not found, a new one is created in the given path with the default contents.
    ///
    /// ### Arguments
    ///
    /// - `config_dir`: the directory where the config file is expected to be
    /// - `shutdown_rx`: optional shutdown signal. If none is provided, a default one will be created, listening for Ctrl-C.
    pub async fn start_from_daemon(
        config_dir: PathBuf,
        shutdown_rx: Option<Receiver<bool>>,
    ) -> Result<(), DynError> {
        let daemon_config = DaemonConfig::read_or_create_config_file(config_dir).await?;
        let watcher_config = WatcherConfig::from(daemon_config);
        NexusWatcherBuilder(watcher_config).start(shutdown_rx).await
    }

    pub async fn start(
        mut shutdown_rx: Receiver<bool>,
        config: WatcherConfig,
    ) -> Result<(), DynError> {
        debug!(?config, "Running NexusWatcher with ");

        // Check if the configured homeserver is persisted in the graph
        let config_hs = PubkyId::try_from(config.homeserver.as_str())?;
        Homeserver::persist_if_unknown(config_hs).await?;

        let event_processor_factory = EventProcessorFactory::from_config(&config);
        let period = Duration::from_millis(config.watcher_sleep);

        let mut cycle_processor = CycleProcessor::new(
            Arc::new(event_processor_factory),
            shutdown_rx.clone(),
            period,
        );

        loop {
            // Process one complete cycle and get elapsed time
            let elapsed = cycle_processor.run_cycle().await?;

            // Check shutdown after cycle completion
            if *shutdown_rx.borrow() {
                info!(
                    "Shutdown requested after cycle {}; exiting.",
                    cycle_processor.cycle_count()
                );
                break;
            }

            // Pace the next cycle with actual elapsed time
            if !cycle_processor.pace_cycle(elapsed, &mut shutdown_rx).await {
                info!("Shutdown during pacing; exiting.");
                break;
            }
        }

        info!(
            total_cycles = cycle_processor.cycle_count(),
            "Watcher shut down gracefully"
        );

        Ok(())
    }
}

/// Manages the periodic processing of homeservers in isolated cycles
struct CycleProcessor {
    event_processor_factory: Arc<EventProcessorFactory>,
    period: Duration,
    cycle: u64,
    timeout: Duration,
    shutdown_rx: Receiver<bool>,
}

impl CycleProcessor {
    fn new(
        event_processor_factory: Arc<EventProcessorFactory>,
        shutdown_rx: Receiver<bool>,
        period: Duration,
    ) -> Self {
        Self {
            event_processor_factory,
            period,
            cycle: 0,
            timeout: Duration::from_secs(PROCESSING_TIMEOUT_SECS),
            shutdown_rx,
        }
    }

    fn cycle_count(&self) -> u64 {
        self.cycle
    }

    /// Executes one full processing cycle
    async fn run_cycle(&mut self) -> Result<Duration, DynError> {
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
    async fn exec_watcher(&self, homeservers: Vec<String>) -> (usize, usize) {
        let mut window = RollingWindow::new(MAX_CONCURRENT);
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
    async fn pace_cycle(&self, elapsed: Duration, shutdown_rx: &mut Receiver<bool>) -> bool {
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

/// Manages a rolling window of concurrent tasks
struct RollingWindow {
    set: JoinSet<Result<(), DynError>>,
    max_size: usize,
}

impl RollingWindow {
    fn new(max_size: usize) -> Self {
        Self {
            set: JoinSet::new(),
            max_size,
        }
    }

    /// Checks if the window is full
    fn is_full(&self) -> bool {
        self.set.len() >= self.max_size
    }

    /// Spawns new tasks into the rolling window until it is full
    fn fill(
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
    async fn join_next(&mut self) -> Option<ProcessResult> {
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

enum ProcessResult {
    Success,
    Error(DynError),
    Panic(tokio::task::JoinError),
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
