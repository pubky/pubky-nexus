use super::TEventProcessorRunner;
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

    /// See [WatcherConfig::homeserver]
    pub primary_homeserver: PubkyId,

    /// Homeservers read as a whole feed, see [`full_feed_homeservers`]
    pub full_feeds: Vec<PubkyId>,

    /// HS PKs that must never be indexed: a listed homeserver on it isn't read
    pub hs_blacklist: HsBlacklist,

    /// Scheduler shared with every processor this runner builds
    pub retry_scheduler: Arc<RetryScheduler>,

    /// The watcher's user ingestor, whose scope decides which authors' events are read
    /// from the feeds
    pub ingestor: Arc<UserIngestor>,
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
            primary_homeserver: config.homeserver.clone(),
            full_feeds: full_feed_homeservers(&config.stack.scope, &config.homeserver),
            hs_blacklist: HsBlacklist::from_config(&config.stack),
            ingestor: Arc::new(watcher_ingestor(config, client.clone())),
            client,
            retry_scheduler,
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
            client: self.client.clone(),
            shutdown_rx: self.shutdown_rx.clone(),
            retry_scheduler: self.retry_scheduler.clone(),
            ingestor: self.ingestor.clone(),
            hs_mapping_cache: Default::default(),
        }))
    }

    /// The homeservers read as a whole feed: the primary one, unless the scope
    /// replaces it with a list. A blacklisted homeserver in the list is left out, as the
    /// key-based runner leaves it out; the primary is read as it always was.
    async fn pre_run(&self) -> Result<Vec<String>, DynError> {
        Ok(self
            .full_feeds
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
