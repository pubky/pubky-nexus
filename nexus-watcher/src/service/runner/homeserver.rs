use super::{TEventProcessorRunner, UserNotFoundBackoff, UserResumePoints};
use crate::events::retry::RetryScheduler;
use crate::events::{DefaultEventHandler, EventHandler};
use crate::service::indexer::{HsEventProcessor, TEventProcessor};
use crate::service::stats::{ProcessedStats, RunAllProcessorsStats};
use nexus_common::models::homeserver::{Homeserver, HsBlacklist};
use nexus_common::models::scope::{scope_from_config, Scope};
use nexus_common::types::DynError;
use nexus_common::WatcherConfig;
use pubky_app_specs::PubkyId;
use std::sync::Arc;
use tokio::sync::watch::Receiver;
use tracing::debug;

pub struct HsEventProcessorRunner {
    /// See [WatcherConfig::events_limit]
    pub limit: u16,

    pub event_handler: Arc<dyn EventHandler>,
    pub shutdown_rx: Receiver<bool>,

    /// See [WatcherConfig::homeserver]
    pub primary_homeserver: PubkyId,

    /// Decides which homeservers' whole feeds are read
    pub scope: Arc<dyn Scope>,

    /// HS PKs that must never be indexed: a listed homeserver on it isn't read
    pub hs_blacklist: HsBlacklist,

    /// Scheduler shared with every processor this runner builds
    pub retry_scheduler: Arc<RetryScheduler>,

    /// Where catch-up runs start (see `HsEventProcessor::catch_up_users`), kept across
    /// polls so a run that stops at its budget leaves the next one the users it didn't reach
    pub catch_up_resume_points: Arc<UserResumePoints>,

    /// Catching-up users whose stream returned 404, kept across polls like the resume points
    pub catch_up_not_found: Arc<UserNotFoundBackoff>,
}

impl HsEventProcessorRunner {
    /// Creates a new instance from the provided configuration
    pub fn from_config(config: &WatcherConfig, shutdown_rx: Receiver<bool>) -> Self {
        Self {
            limit: config.events_limit,
            event_handler: Arc::new(DefaultEventHandler::from_config(config)),
            shutdown_rx,
            primary_homeserver: config.homeserver.clone(),
            scope: scope_from_config(&config.stack.scope),
            hs_blacklist: HsBlacklist::from_config(&config.stack),
            retry_scheduler: Arc::new(RetryScheduler::from_config(config)),
            catch_up_resume_points: Arc::new(UserResumePoints::default()),
            catch_up_not_found: Arc::new(UserNotFoundBackoff::default()),
        }
    }

    pub fn primary_homeserver(&self) -> &str {
        &self.primary_homeserver
    }
}

#[async_trait::async_trait]
impl TEventProcessorRunner for HsEventProcessorRunner {
    fn shutdown_rx(&self) -> Receiver<bool> {
        self.shutdown_rx.clone()
    }

    /// Creates and returns a new event processor instance for the specified homeserver
    async fn build(&self, homeserver_id: &str) -> Result<Arc<dyn TEventProcessor>, DynError> {
        let homeserver_id = PubkyId::try_from(homeserver_id)?;
        let homeserver = Homeserver::get_by_id(homeserver_id)
            .await?
            .ok_or("Homeserver not found")?;

        Ok(Arc::new(HsEventProcessor {
            homeserver,
            limit: self.limit,
            event_handler: self.event_handler.clone(),
            shutdown_rx: self.shutdown_rx.clone(),
            retry_scheduler: self.retry_scheduler.clone(),
            catch_up_resume_points: self.catch_up_resume_points.clone(),
            catch_up_not_found: self.catch_up_not_found.clone(),
            hs_mapping_cache: Default::default(),
        }))
    }

    /// The homeservers read as a whole feed: the primary one, unless the scope
    /// replaces it with a list. A blacklisted homeserver in the list is left out, as the
    /// key-based runner leaves it out; the primary is read as it always was.
    async fn pre_run(&self) -> Result<Vec<String>, DynError> {
        let homeservers = self.scope.full_feed_homeservers(&self.primary_homeserver);
        Ok(homeservers
            .iter()
            .filter(|hs| {
                **hs == self.primary_homeserver || !self.hs_blacklist.is_blacklisted(hs.as_ref())
            })
            .map(ToString::to_string)
            .collect())
    }

    async fn post_run(&self, stats: RunAllProcessorsStats) -> ProcessedStats {
        for individual_run_stat in &stats.stats {
            let hs_id = &individual_run_stat.hs_id;
            let duration = individual_run_stat.duration;
            let status = &individual_run_stat.status;
            debug!(homeserver = %hs_id, ?duration, ?status, "Primary homeserver run completed");
        }

        ProcessedStats(stats)
    }
}
