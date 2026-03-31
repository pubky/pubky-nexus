use super::TEventProcessorRunner;
use crate::events::Moderation;
use crate::service::backoff::HomeserverBackoff;
use crate::service::indexer::{KeyBasedEventProcessor, TEventProcessor};
use crate::service::runner::status_from_run_result;
use crate::service::stats::{ProcessedStats, ProcessorRunStatus, RunAllProcessorsStats};
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use nexus_common::WatcherConfig;
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{watch::Receiver, Mutex};
use tracing::{debug, error, info, warn};

pub struct KeyBasedEventProcessorRunner {
    /// See [WatcherConfig::events_limit]
    pub limit: u32,
    /// See [WatcherConfig::monitored_homeservers_limit]
    pub monitored_hs_limit: usize,
    pub files_path: PathBuf,
    pub tracer_name: String,
    pub moderation: Arc<Moderation>,
    pub shutdown_rx: Receiver<bool>,
    /// Default homeserver ID, excluded from the external targets list
    pub default_homeserver: PubkyId,
    /// Per-target exponential backoff state
    pub backoff: Mutex<HomeserverBackoff>,
}

impl KeyBasedEventProcessorRunner {
    /// Creates a new instance from the provided configuration
    pub fn from_config(config: &WatcherConfig, shutdown_rx: Receiver<bool>) -> Self {
        Self {
            limit: config.events_limit,
            monitored_hs_limit: config.monitored_homeservers_limit,
            files_path: config.stack.files_path.clone(),
            tracer_name: config.name.clone(),
            moderation: Arc::new(Moderation {
                id: config.moderation_id.clone(),
                tags: config.moderated_tags.clone(),
            }),
            shutdown_rx,
            default_homeserver: config.homeserver.clone(),
            backoff: Mutex::new(HomeserverBackoff::new(
                config.initial_backoff_secs,
                config.max_backoff_secs,
            )),
        }
    }

    /// Returns the homeserver IDs relevant for this run, ordered by their priority.
    /// The default homeserver is excluded from this list.
    async fn hs_by_priority(&self) -> Result<Vec<String>, DynError> {
        let hs_ids = Homeserver::get_all_from_graph().await?;
        let default_hs = self.default_homeserver.as_str();

        let hs_ids: Vec<String> = hs_ids
            .into_iter()
            .filter(|hs_id| hs_id != default_hs)
            .collect();

        Ok(hs_ids)
    }
}

#[async_trait::async_trait]
impl TEventProcessorRunner for KeyBasedEventProcessorRunner {
    fn shutdown_rx(&self) -> Receiver<bool> {
        self.shutdown_rx.clone()
    }

    async fn build(&self, hs_id: String) -> Result<Arc<dyn TEventProcessor>, DynError> {
        let homeserver_id = PubkyId::try_from(&hs_id)?;
        let homeserver = Homeserver::get_by_id(homeserver_id)
            .await?
            .ok_or("Homeserver not found")?;

        Ok(Arc::new(KeyBasedEventProcessor {
            homeserver,
            files_path: self.files_path.clone(),
            tracer_name: self.tracer_name.clone(),
            moderation: self.moderation.clone(),
        }))
    }

    async fn pre_run(&self) -> Result<Vec<String>, DynError> {
        let hs_ids = self.hs_by_priority().await?;
        let max_index = std::cmp::min(self.monitored_hs_limit, hs_ids.len());
        Ok(hs_ids[..max_index].to_vec())
    }

    async fn run(&self) -> Result<ProcessedStats, DynError> {
        let hs_ids = self.pre_run().await?;
        let mut run_stats = RunAllProcessorsStats::default();

        for hs_id in hs_ids {
            if *self.shutdown_rx().borrow() {
                info!(hs_id = %hs_id, "Shutdown detected; exiting run loop");
                break;
            }

            {
                let backoff = self.backoff.lock().await;
                if backoff.should_skip(&hs_id) {
                    debug!(hs_id = %hs_id, "Skipping homeserver in backoff");
                    run_stats.add_run_result(
                        hs_id,
                        std::time::Duration::ZERO,
                        ProcessorRunStatus::Skipped,
                    );
                    continue;
                }
            }

            let t0 = Instant::now();
            let status = match self.build(hs_id.clone()).await {
                Ok(event_processor) => status_from_run_result(event_processor.run().await),
                Err(e) => {
                    error!(hs_id = %hs_id, error = %e, "Failed to build event processor");
                    ProcessorRunStatus::FailedToBuild
                }
            };
            let duration = t0.elapsed();

            {
                let mut backoff = self.backoff.lock().await;
                if status == ProcessorRunStatus::Ok {
                    backoff.record_success(&hs_id);
                } else {
                    backoff.record_failure(&hs_id);
                }
            }

            run_stats.add_run_result(hs_id, duration, status);
        }

        let processed_stats = self.post_run(run_stats).await;
        Ok(processed_stats)
    }

    async fn post_run(&self, stats: RunAllProcessorsStats) -> ProcessedStats {
        for individual_run_stat in &stats.stats {
            let hs_id = &individual_run_stat.hs_id;
            let duration = individual_run_stat.duration;
            let status = &individual_run_stat.status;
            debug!(
                hs_id = %hs_id,
                duration = ?duration,
                status = ?status,
                "Event processor run completed"
            );
        }

        let count_ok = stats.count_ok();
        let count_error = stats.count_error();
        let count_panic = stats.count_panic();
        let count_timeout = stats.count_timeout();
        let count_failed_to_build = stats.count_failed_to_build();
        let count_skipped = stats.count_skipped();
        let had_issues = count_error + count_panic + count_timeout + count_failed_to_build > 0;

        if had_issues {
            warn!("Run result: {count_ok} ok, {count_skipped} skipped (backoff), {count_failed_to_build} failed to build, {count_error} error, {count_panic} panic, {count_timeout} timeout");
        } else if count_skipped > 0 {
            info!("Run result: {count_ok} ok, {count_skipped} skipped (backoff)");
        } else {
            debug!("Run result: {count_ok} ok");
        }

        ProcessedStats(stats)
    }
}
