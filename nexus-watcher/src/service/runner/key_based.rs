use super::TEventProcessorRunner;
use crate::events::Moderation;
use crate::service::backoff::HomeserverBackoff;
use crate::service::indexer::{KeyBasedEventProcessor, TEventProcessor};
use crate::service::stats::{ProcessedStats, ProcessorRunStatus, RunAllProcessorsStats};
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use nexus_common::WatcherConfig;
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{watch::Receiver, Mutex};
use tracing::{debug, info, warn};

/// Runner for [KeyBasedEventProcessor]
pub struct KeyBasedEventProcessorRunner {
    /// See [WatcherConfig::events_limit]
    pub limit: u32,
    /// See [WatcherConfig::monitored_homeservers_limit]
    pub monitored_hs_limit: usize,
    pub files_path: PathBuf,
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
        let hs_ids = Homeserver::get_all_active_from_graph().await?;
        let default_hs = self.default_homeserver.as_str();

        // Exclude the default homeserver from the list, as it is processed separately
        // The default HS is not expected to be active, but we still filter as an extra precaution
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
            moderation: self.moderation.clone(),
        }))
    }

    async fn pre_run(&self) -> Result<Vec<String>, DynError> {
        let hs_ids = self.hs_by_priority().await?;
        let max_index = std::cmp::min(self.monitored_hs_limit, hs_ids.len());
        Ok(hs_ids[..max_index].to_vec())
    }

    async fn backoff_should_skip(&self, hs_id: &str) -> Option<ProcessorRunStatus> {
        let backoff = self.backoff.lock().await;
        if backoff.should_skip(hs_id) {
            debug!(hs_id = %hs_id, "Skipping homeserver in backoff");
            Some(ProcessorRunStatus::Skipped)
        } else {
            None
        }
    }

    async fn backoff_on_result(&self, hs_id: &str, status: &ProcessorRunStatus) {
        let mut backoff = self.backoff.lock().await;
        if *status == ProcessorRunStatus::Ok {
            backoff.record_success(hs_id);
        } else {
            backoff.record_failure(hs_id);
        }
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
