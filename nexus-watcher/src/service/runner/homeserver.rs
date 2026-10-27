use super::{TEventProcessorRunner, UserNotFoundBackoff, UserResumePoints};
use crate::events::retry::RetryScheduler;
use crate::events::EventHandler;
use crate::homeserver::HomeserverClient;
use crate::scope::{full_feed_homeservers, watcher_ingestor};
use crate::service::indexer::{HsEventProcessor, TEventProcessor};
use crate::service::stats::{ProcessedStats, RunAllProcessorsStats};
use nexus_common::models::homeserver::{Homeserver, HsBlacklist};
use nexus_common::models::user::UserIngestor;
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
    pub client: Arc<dyn HomeserverClient>,
    pub shutdown_rx: Receiver<bool>,

    /// Homeservers read as a whole feed, see [`full_feed_homeservers`]
    pub full_feeds: Vec<PubkyId>,

    /// Scheduler shared with every processor this runner builds
    pub retry_scheduler: Arc<RetryScheduler>,

    /// Where catch-up runs start (see `HsEventProcessor::catch_up_users`), kept across
    /// polls so a run that stops at its budget leaves the next one the users it didn't reach
    pub catch_up_resume_points: Arc<UserResumePoints>,

    /// Catching-up users whose stream returned 404, kept across polls like the resume points
    pub catch_up_not_found: Arc<UserNotFoundBackoff>,

    /// The watcher's user ingestor, whose scope decides which authors' events are read
    /// from the feeds
    pub ingestor: Arc<UserIngestor>,

    /// The configured HS blacklist, handed to every processor this runner builds
    pub hs_blacklist: HsBlacklist,
}

impl HsEventProcessorRunner {
    /// Creates a runner configured by `config`, sharing the given client, handler and scheduler
    pub fn new(
        config: &WatcherConfig,
        client: Arc<dyn HomeserverClient>,
        event_handler: Arc<dyn EventHandler>,
        retry_scheduler: Arc<RetryScheduler>,
        shutdown_rx: Receiver<bool>,
    ) -> Self {
        Self {
            limit: config.events_limit,
            event_handler,
            shutdown_rx,
            full_feeds: full_feed_homeservers(config),
            ingestor: Arc::new(watcher_ingestor(config, client.clone())),
            client,
            retry_scheduler,
            catch_up_resume_points: Arc::new(UserResumePoints::default()),
            catch_up_not_found: Arc::new(UserNotFoundBackoff::default()),
            hs_blacklist: HsBlacklist::from_config(&config.stack),
        }
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
            client: self.client.clone(),
            shutdown_rx: self.shutdown_rx.clone(),
            retry_scheduler: self.retry_scheduler.clone(),
            catch_up_resume_points: self.catch_up_resume_points.clone(),
            catch_up_not_found: self.catch_up_not_found.clone(),
            ingestor: self.ingestor.clone(),
            hs_blacklist: self.hs_blacklist.clone(),
            hs_mapping_cache: Default::default(),
        }))
    }

    /// The homeservers read as a whole feed, see [`full_feed_homeservers`].
    async fn pre_run(&self) -> Result<Vec<String>, DynError> {
        Ok(self.full_feeds.iter().map(ToString::to_string).collect())
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
