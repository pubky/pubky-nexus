use anyhow::Result;
use nexus_common::models::event::EventProcessorError;
use nexus_common::types::DynError;
use nexus_watcher::service::{NexusWatcher, TEventProcessor, TEventProcessorRunner};
use pubky_app_specs::PubkyId;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::watch::Receiver;

const TEST_HS_ID: &str = "1hb71xx9km3f4pw5izsy1gn19ff1uuuqonw4mcygzobwkryujoiy";

/// Processor that sleeps on the first N invocations and is instant afterward.
/// This pattern is essential: `MissedTickBehavior::Skip` only produces
/// observable waiting when processing time drops *below* the interval after
/// a slow round. When every round exceeds the interval, Skip degenerates to
/// immediate-fire behavior identical to Burst.
struct VariableDelayProcessor {
    call_count: AtomicUsize,
    slow_first_n: usize,
    slow_delay: Duration,
    hs_id: PubkyId,
}

#[async_trait::async_trait]
impl TEventProcessor for VariableDelayProcessor {
    fn get_homeserver_id(&self) -> PubkyId {
        self.hs_id.clone()
    }

    async fn run_internal(self: Arc<Self>) -> Result<(), EventProcessorError> {
        let n = self.call_count.fetch_add(1, Ordering::SeqCst);
        if n < self.slow_first_n {
            tokio::time::sleep(self.slow_delay).await;
        }
        Ok(())
    }
}

/// Minimal runner that returns the shared `VariableDelayProcessor` on each
/// `build()` call. External homeservers are empty so only the default-HS
/// loop triggers builds.
struct VariableDelayRunner {
    processor: Arc<VariableDelayProcessor>,
    build_timestamps: std::sync::Mutex<Vec<Instant>>,
    shutdown_rx: Receiver<bool>,
}

#[async_trait::async_trait]
impl TEventProcessorRunner for VariableDelayRunner {
    fn shutdown_rx(&self) -> Receiver<bool> {
        self.shutdown_rx.clone()
    }

    fn default_homeserver(&self) -> &str {
        TEST_HS_ID
    }

    fn monitored_homeservers_limit(&self) -> usize {
        0
    }

    async fn external_homeservers_by_priority(&self) -> Result<Vec<String>, DynError> {
        Ok(vec![])
    }

    /// Records the instant of each `build()` call, then returns the shared processor.
    ///
    /// `NexusWatcher::run_tasks` calls `build()` on each interval tick for the
    /// default homeserver loop. By collecting these timestamps we can assert that
    /// calls stay spaced out (Skip) instead of clustering in burst catch-up mode.
    async fn build(&self, _homeserver_id: String) -> Result<Arc<dyn TEventProcessor>, DynError> {
        self.build_timestamps.lock().unwrap().push(Instant::now());
        Ok(self.processor.clone())
    }
}

/// Verifies that `run_tasks` uses `MissedTickBehavior::Skip`.
///
/// The first processing round sleeps 500 ms (exceeding the 100 ms interval),
/// causing several ticks to be missed. Subsequent rounds are instant.
///
/// With **Skip**, missed ticks are dropped and the loop resumes with regular
/// spacing, i.e. no immediate back-to-back `build()` calls.
///
/// With **Burst** (tokio default), missed ticks fire immediately after the
/// slow round and create near-zero inter-build gaps.
#[tokio_shared_rt::test(shared)]
async fn test_no_burst_after_slow_processing_round() -> Result<()> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let processor = Arc::new(VariableDelayProcessor {
        call_count: AtomicUsize::new(0),
        slow_first_n: 1,
        slow_delay: Duration::from_millis(500),
        hs_id: PubkyId::try_from(TEST_HS_ID).unwrap(),
    });

    let runner = Arc::new(VariableDelayRunner {
        processor,
        build_timestamps: std::sync::Mutex::new(Vec::new()),
        shutdown_rx: shutdown_rx.clone(),
    });

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1500)).await;
        let _ = shutdown_tx.send(true);
    });

    let _ = NexusWatcher::run_tasks(shutdown_rx, runner.clone(), 100).await;

    let timestamps = runner.build_timestamps.lock().unwrap().clone();

    assert!(
        timestamps.len() >= 3,
        "Expected at least 3 build() calls, got {}",
        timestamps.len()
    );

    // We do not require perfect 100 ms cadence because test hosts can be noisy,
    // but any "burst catch-up" would produce near-zero gaps. Keep a conservative
    // floor that catches burst behavior without being CI-fragile.
    let min_expected_non_burst_gap = Duration::from_millis(30);

    for i in 1..timestamps.len() {
        let gap = timestamps[i].duration_since(timestamps[i - 1]);
        assert!(
            gap >= min_expected_non_burst_gap,
            "Gap between build() call {} and {} was {gap:?}, expected >= {min_expected_non_burst_gap:?}. \
             This suggests burst catch-up ticks fired instead of being skipped.",
            i - 1,
            i,
        );
    }

    Ok(())
}
