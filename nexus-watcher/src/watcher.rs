use crate::events::processor::EventProcessor;
use crate::NexusWatcherBuilder;
use nexus_common::file::ConfigLoader;
use nexus_common::types::DynError;
use nexus_common::utils::create_shutdown_rx;
use nexus_common::{DaemonConfig, WatcherConfig};
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::watch::Receiver;
use tokio::task::JoinSet;
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

        let cfg = Arc::new(config);
        let period = Duration::from_millis(cfg.watcher_sleep);

        let mut cycle_processor = CycleProcessor::new(cfg, period);

        loop {
            // Process one complete cycle and get elapsed time
            // TODO: How to avoid to clone the shutdown_rx?
            let elapsed = cycle_processor.run_cycle(shutdown_rx.clone()).await;

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

/// Handles the processing of homeservers in cycles
struct CycleProcessor {
    cfg: Arc<WatcherConfig>,
    period: Duration,
    cycle: u64,
    timeout: Duration,
}

impl CycleProcessor {
    fn new(cfg: Arc<WatcherConfig>, period: Duration) -> Self {
        let timeout = Duration::from_secs(PROCESSING_TIMEOUT_SECS);
        Self {
            cfg,
            period,
            cycle: 0,
            timeout,
        }
    }

    fn cycle_count(&self) -> u64 {
        self.cycle
    }

    async fn run_cycle(&mut self, shutdown_rx: Receiver<bool>) -> Duration {
        self.cycle += 1;
        let cycle_start = Instant::now();

        info!(cycle = self.cycle, "Starting cycle");

        // TODO: Load homeservers from db
        let homeservers: Vec<String> = vec![
            String::from("8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty"),
            String::from("ufibwbmed6jeq9k4p583go95wofakh9fwpp4k734trq79pd9u1uy"),
        ];

        info!(
            cycle = self.cycle,
            count = homeservers.len(),
            "Processing homeservers"
        );

        // Process with rolling window
        let (ok, fail) = self.process_homeservers(homeservers, shutdown_rx).await;

        let elapsed = cycle_start.elapsed();
        info!(
            cycle = self.cycle,
            ok,
            fail,
            elapsed_ms = elapsed.as_millis(),
            "Cycle finished"
        );

        elapsed
    }

    async fn process_homeservers(
        &self,
        homeservers: Vec<String>,
        shutdown_rx: Receiver<bool>,
    ) -> (usize, usize) {
        let mut window = RollingWindow::new(MAX_CONCURRENT);
        let mut iter = homeservers.into_iter();

        // Statistics
        let mut ok = 0usize;
        let mut fail = 0usize;

        // Prime the window
        window.fill(&mut iter, &self.cfg, self.timeout, shutdown_rx.clone());

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
            window.fill(&mut iter, &self.cfg, self.timeout, shutdown_rx.clone());
        }

        (ok, fail)
    }

    async fn pace_cycle(&self, elapsed: Duration, shutdown_rx: &mut Receiver<bool>) -> bool {
        if elapsed < self.period {
            let remaining = self.period - elapsed;
            warn!(
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

    fn fill(
        &mut self,
        iter: &mut impl Iterator<Item = String>,
        cfg: &Arc<WatcherConfig>,
        timeout: Duration,
        shutdown_rx: Receiver<bool>,
    ) {
        // Check how many tasks are currently active in the JoinSet
        while self.set.len() < self.max_size {
            match iter.next() {
                Some(homeserver_id) => {
                    self.set.spawn(process_homeserver(
                        homeserver_id,
                        cfg.clone(),
                        timeout,
                        shutdown_rx.clone(),
                    ));
                }
                None => break,
            }
        }
    }

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

/// Process a single homeserver with timeout
async fn process_homeserver(
    homeserver_id: String,
    cfg: Arc<WatcherConfig>,
    timeout: Duration,
    shutdown_rx: Receiver<bool>,
) -> Result<(), DynError> {
    let start = Instant::now();

    // Initialize processor
    // TODO: How to solve homeserver_id? Shall we read from the config file or from the db?
    // - Now we get the homeserver from the config file but when we do discover new homeservers, we need to read from the db
    // - Create some mechanism to read default one from file, others from db in the initial phase
    let mut processor = match EventProcessor::from_config(&cfg, homeserver_id.clone()).await {
        Ok(p) => p,
        Err(e) => {
            error!(homeserver = %homeserver_id, error = %e, "Failed to init EventProcessor");
            return Err(e);
        }
    };

    // Process with timeout
    match tokio::time::timeout(timeout, processor.run(shutdown_rx)).await {
        Ok(Ok(())) => {
            debug!(
                homeserver = %homeserver_id,
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
                timeout_secs = timeout.as_secs(),
                "Processing timeout"
            );
            Err("processing timeout".into())
        }
    }
}

//pub async fn start(
//     mut shutdown_rx: Receiver<bool>,
//     config: WatcherConfig,
// ) -> Result<(), DynError> {
//     debug!(?config, "Running NexusWatcher with ");

//     let semaphore = Arc::new(Semaphore::new(5));
//     let mut interval = tokio::time::interval(Duration::from_millis(config.watcher_sleep));

//     loop {
//         tokio::select! {
//             _ = shutdown_rx.changed() => {
//                 info!("SIGINT received, starting graceful shutdown...");
//                 break;
//             }
//             // SOMEHOW WE NEED TO CONTROL BEFORE THE INTERVAL TICK ALL THE HOMESERVERS DID THEIR JOB
//             //
//             _ = interval.tick() => {
//                 info!("----- Interval tick starting again ------");
//                 // TODO: Get homeservers from db
//                 let homeserver_ids = [
//                     // PRODUCTION hs
//                     "8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty",
//                     // STAGING hs
//                     "ufibwbmed6jeq9k4p583go95wofakh9fwpp4k734trq79pd9u1uy",
//                     // LOCAL hs
//                     "8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo",
//                 ];
//                 let tasks: Vec<_> = homeserver_ids.into_iter().map(|homeserver_id| {
//                     let semaphore = semaphore.clone();
//                     let shutdown_rx = shutdown_rx.clone();
//                     let config = config.clone();
//                     tokio::spawn(async move {
//                         let _permit = semaphore.acquire().await;
//                         let mut processor = EventProcessor::from_config(&config).await?;
//                         // TODO wrong
//                         processor.homeserver.id = PubkyId::try_from(homeserver_id).unwrap();
//                         debug!(?processor.homeserver.id, "----- Starting event processing… ------");
//                         processor.run(shutdown_rx).await
//                     })
//                 }).collect();
//                 // Wait for all to complete
//                 for task in tasks {
//                     match task.await {
//                         Ok(result) => {
//                             if let Err(e) = result {
//                                 error!("xxxxxx Task failed: {:?}", e);
//                             } else {
//                                 warn!("Task completed: {:?}", result);
//                             }
//                         }
//                         // This only catches tokio task panics, not business logic errors
//                         Err(e) => {
//                             error!("Task panicked: {:?}", e);
//                         }
//                     }
//                 }
//             }
//         }
//     }
//     info!("Nexus Watcher shut down gracefully");
//     Ok(())
// }
