mod homeserver;
mod key_based;
mod key_based_hs_backoff;
mod key_based_user_backoff;
mod key_based_user_resume;

pub use homeserver::HsEventProcessorRunner;
pub use key_based::KeyBasedEventProcessorRunner;
pub use key_based_hs_backoff::HomeserverBackoff;
pub use key_based_user_backoff::UserNotFoundBackoff;
pub use key_based_user_resume::UserResumePoints;

pub use pubky_watcher::{status_from_run_result, TEventProcessorRunner};

#[cfg(test)]
mod tests {
    use super::HomeserverBackoff;
    use super::*;
    use crate::errors::EventProcessorError;
    use crate::events::{Event, EventHandler};
    use crate::service::indexer::{NexusEventProcessor, RunError};
    use crate::service::indexer::{RunOutcome, TEventProcessor};
    use crate::service::stats::{ProcessedStats, ProcessorRunStatus};
    use nexus_common::types::DynError;
    use nexus_common::utils::test_utils::random_pubky_id;
    use pubky_social_specs::PubkyId;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::watch::Receiver;
    use tokio::sync::Mutex;

    struct NoopHandler;

    #[async_trait::async_trait]
    impl EventHandler for NoopHandler {
        async fn handle(
            &self,
            _event: &Event,
            _homeserver_id: &PubkyId,
        ) -> Result<(), EventProcessorError> {
            Ok(())
        }
    }

    /// Processor whose run takes `work`, and is slow past `slow_run_threshold`. It counts the most
    /// of its runs going at once.
    struct SlowProcessor {
        handler: Arc<dyn EventHandler>,
        homeserver_id: PubkyId,
        work: Duration,
        slow_run_threshold: Duration,
        running: AtomicUsize,
        peak: AtomicUsize,
    }

    impl SlowProcessor {
        fn new(work: Duration, slow_run_threshold: Duration) -> Self {
            Self {
                handler: Arc::new(NoopHandler),
                homeserver_id: random_pubky_id(),
                work,
                slow_run_threshold,
                running: AtomicUsize::new(0),
                peak: AtomicUsize::new(0),
            }
        }

        /// The most runs that were going at once.
        fn peak(&self) -> usize {
            self.peak.load(Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl TEventProcessor<Event, EventProcessorError> for SlowProcessor {
        async fn handle(&self, event: &Event) -> Result<(), EventProcessorError> {
            self.handler.handle(event, &self.homeserver_id).await
        }

        fn instance_name(&self) -> &'static str {
            "SlowProcessor"
        }

        fn homeserver_id(&self) -> &str {
            self.homeserver_id.as_ref()
        }

        fn slow_run_threshold(&self) -> Option<Duration> {
            Some(self.slow_run_threshold)
        }

        async fn run_internal(self: Arc<Self>) -> Result<RunOutcome, EventProcessorError> {
            let running = self.running.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(running, Ordering::SeqCst);
            tokio::time::sleep(self.work).await;
            self.running.fetch_sub(1, Ordering::SeqCst);
            Ok(RunOutcome::Completed)
        }
    }

    /// Runner over fixed HS IDs that records which ones it was asked to build.
    struct RecordingRunner {
        hs_ids: Vec<String>,
        poll_limit: usize,
        backoff: HomeserverBackoff,
        built: Mutex<Vec<String>>,
        /// The processor every build returns. Builds fail unless it is set.
        processor: Option<Arc<SlowProcessor>>,
        shutdown_rx: Receiver<bool>,
    }

    impl RecordingRunner {
        fn new(hs_ids: &[&str], poll_limit: usize) -> Self {
            Self {
                hs_ids: hs_ids.iter().map(|id| id.to_string()).collect(),
                poll_limit,
                backoff: HomeserverBackoff::default(),
                built: Mutex::new(vec![]),
                processor: None,
                shutdown_rx: tokio::sync::watch::channel(false).1,
            }
        }

        /// Builds return one processor, whose runs take `work` and are slow past
        /// `slow_run_threshold`.
        fn with_processors_taking(mut self, work: Duration, slow_run_threshold: Duration) -> Self {
            self.processor = Some(Arc::new(SlowProcessor::new(work, slow_run_threshold)));
            self
        }
    }

    #[async_trait::async_trait]
    impl TEventProcessorRunner<Event, EventProcessorError> for RecordingRunner {
        type Error = DynError;

        fn shutdown_rx(&self) -> Receiver<bool> {
            self.shutdown_rx.clone()
        }

        async fn build(&self, hs_id: &str) -> Result<Arc<NexusEventProcessor>, DynError> {
            self.built.lock().await.push(hs_id.to_string());
            let processor = self.processor.clone().ok_or("not built in this test")?;
            Ok(processor)
        }

        async fn pre_run(&self) -> Result<Vec<String>, DynError> {
            Ok(self.hs_ids.clone())
        }

        fn poll_limit(&self) -> usize {
            self.poll_limit
        }

        async fn backoff_hs_should_skip(&self, hs_id: &str) -> bool {
            self.backoff.should_skip(hs_id)
        }
    }

    #[tokio::test]
    async fn poll_limit_caps_the_homeservers_built() {
        let runner = RecordingRunner::new(&["hs1", "hs2", "hs3", "hs4"], 2);

        runner.run().await.unwrap();

        assert_eq!(*runner.built.lock().await, ["hs1", "hs2"]);
    }

    #[tokio::test]
    async fn backed_off_homeservers_do_not_use_up_the_poll_limit() {
        let runner = RecordingRunner::new(&["hs1", "hs2", "hs3", "hs4", "hs5"], 2);
        runner.backoff.record_failure("hs1");
        runner.backoff.record_failure("hs2");

        let ProcessedStats(stats) = runner.run().await.unwrap();

        // The two highest-priority HSs are in backoff, so the next two take their slots
        assert_eq!(*runner.built.lock().await, ["hs3", "hs4"]);
        assert_eq!(stats.count_skipped(), 2);
        assert_eq!(stats.count_failed_to_build(), 2);
    }

    /// Back-to-back ticks, as the periodic task runs them, never run one homeserver twice at
    /// once, even when its run is slow: the tick waits for the run to end.
    #[tokio::test(start_paused = true)]
    async fn a_homeserver_never_runs_twice_at_once() {
        let runner = RecordingRunner::new(&["hs1"], usize::MAX)
            .with_processors_taking(Duration::from_millis(300), Duration::from_millis(50));

        let ProcessedStats(first) = runner.run().await.unwrap();
        let ProcessedStats(second) = runner.run().await.unwrap();

        let processor = runner.processor.as_ref().expect("set above");
        assert_eq!(processor.peak(), 1, "two runs of hs1 were going at once");
        assert_eq!(
            first.count_ok() + second.count_ok(),
            2,
            "both runs finished"
        );
    }

    /// Processor whose every run stops at its budget.
    struct BudgetProcessor {
        handler: Arc<dyn EventHandler>,
        homeserver_id: PubkyId,
    }

    #[async_trait::async_trait]
    impl TEventProcessor<Event, EventProcessorError> for BudgetProcessor {
        async fn handle(&self, event: &Event) -> Result<(), EventProcessorError> {
            self.handler.handle(event, &self.homeserver_id).await
        }

        fn instance_name(&self) -> &'static str {
            "BudgetProcessor"
        }

        fn homeserver_id(&self) -> &str {
            self.homeserver_id.as_ref()
        }

        async fn run_internal(self: Arc<Self>) -> Result<RunOutcome, EventProcessorError> {
            Ok(RunOutcome::BudgetSpent)
        }
    }

    /// A run that stops at its budget is reported as such, apart from success and failure.
    #[tokio::test]
    async fn a_spent_budget_is_its_own_run_status() {
        let processor = Arc::new(BudgetProcessor {
            handler: Arc::new(NoopHandler),
            homeserver_id: random_pubky_id(),
        });

        let result = processor.run().await;

        assert!(matches!(result, Err(RunError::BudgetSpent)), "{result:?}");
        assert_eq!(
            status_from_run_result(result),
            ProcessorRunStatus::BudgetSpent
        );
    }

    /// Pins a known limitation: the tick waits for every run, so a run that never returns
    /// holds up the homeservers after it for good. Bounding the calls a run makes fixes it.
    #[tokio::test(start_paused = true)]
    async fn a_run_that_never_returns_holds_up_the_homeservers_after_it() {
        let runner = RecordingRunner::new(&["hs1", "hs2"], usize::MAX)
            .with_processors_taking(Duration::MAX, Duration::from_secs(60));

        let tick = tokio::time::timeout(Duration::from_secs(24 * 3600), runner.run()).await;

        assert!(tick.is_err(), "the tick ended");
        assert_eq!(*runner.built.lock().await, ["hs1"]);
    }
}
