use super::{TEventProcessorRunner, UserNotFoundBackoff};
use crate::events::retry::RetryScheduler;
use crate::events::{DefaultEventHandler, EventHandler};
use crate::service::indexer::{
    KeyBasedEventProcessor, KeyBasedEventSource, PubkyKeyBasedEventSource, TEventProcessor,
    METER_NAME,
};
use crate::service::runner::key_based_hs_backoff::HomeserverBackoff;
use crate::service::stats::{ProcessedStats, ProcessorRunStatus, RunAllProcessorsStats};
use nexus_common::models::homeserver::{Homeserver, HsBlacklist};
use nexus_common::types::DynError;
use nexus_common::WatcherConfig;
use opentelemetry::global;
use opentelemetry::metrics::{Gauge, Meter};
use pubky_app_specs::PubkyId;
use std::sync::{Arc, LazyLock};
use tokio::sync::{watch::Receiver, Mutex};
use tracing::{debug, info, warn};

/// Metrics for the external-HS monitoring loop, recorded once per run in
/// [`KeyBasedEventProcessorRunner::pre_run`].
///
/// Instruments come from the global meter, so they are no-ops until an
/// `SdkMeterProvider` is installed.
struct ExternalHsMetrics {
    /// Configured cap, see [WatcherConfig::monitored_homeservers_limit].
    monitored_limit: Gauge<u64>,
    /// External homeservers the last run selected for indexing.
    indexed: Gauge<u64>,
}

impl ExternalHsMetrics {
    /// Builds the instruments from the global meter.
    fn new() -> Self {
        Self::with_meter(global::meter(METER_NAME))
    }

    /// Builds the instruments from an explicit meter, for tests.
    fn with_meter(meter: Meter) -> Self {
        Self {
            monitored_limit: meter
                .u64_gauge("watcher.external_hs.monitored_limit")
                .with_description("Configured cap on the external homeservers monitored per run")
                .build(),
            indexed: meter
                .u64_gauge("watcher.external_hs.indexed")
                .with_description("External homeservers selected for indexing in the last run")
                .build(),
        }
    }

    /// Records one run: the cap in force and the homeservers it selected.
    fn record_run(&self, monitored_limit: usize, indexed: usize) {
        self.monitored_limit.record(monitored_limit as u64, &[]);
        self.indexed.record(indexed as u64, &[]);
    }
}

/// Exported on every external-HS run. A gauge keeps its last value while the
/// process is alive, so `watcher.external_hs.indexed` over
/// `watcher.external_hs.monitored_limit` is the saturation ratio: it reaches 1
/// exactly when the cap is what stops the least trusted external homeservers
/// from being indexed.
static EXTERNAL_HS_METRICS: LazyLock<ExternalHsMetrics> = LazyLock::new(ExternalHsMetrics::new);

/// Runner for [KeyBasedEventProcessor]
pub struct KeyBasedEventProcessorRunner {
    /// See [WatcherConfig::key_based_events_limit]
    pub limit: u16,

    /// See [WatcherConfig::monitored_homeservers_limit]
    pub monitored_hs_limit: usize,

    pub event_handler: Arc<dyn EventHandler>,
    pub event_source: Arc<dyn KeyBasedEventSource>,
    pub shutdown_rx: Receiver<bool>,

    /// Primary homeserver ID, excluded from the external targets list
    pub primary_homeserver: PubkyId,

    /// HS PKs that must never be indexed. Excluded from `pre_run` and re-checked
    /// by each [`KeyBasedEventProcessor`] this runner builds.
    pub hs_blacklist: HsBlacklist,

    /// Per-target exponential backoff state
    pub backoff: Mutex<HomeserverBackoff>,

    pub user_not_found_backoff: Arc<UserNotFoundBackoff>,

    /// Scheduler shared with every processor this runner builds
    pub retry_scheduler: Arc<RetryScheduler>,
}

impl KeyBasedEventProcessorRunner {
    /// Creates a new instance from the provided configuration
    pub fn from_config(config: &WatcherConfig, shutdown_rx: Receiver<bool>) -> Self {
        Self {
            limit: config.key_based_events_limit,
            monitored_hs_limit: config.monitored_homeservers_limit,
            event_handler: Arc::new(DefaultEventHandler::from_config(config)),
            event_source: Arc::new(PubkyKeyBasedEventSource),
            shutdown_rx,
            primary_homeserver: config.homeserver.clone(),
            hs_blacklist: HsBlacklist::from_config(&config.stack),
            backoff: Mutex::new(HomeserverBackoff::new(
                config.initial_backoff_secs,
                config.max_backoff_secs,
            )),
            user_not_found_backoff: Arc::new(UserNotFoundBackoff::default()),
            retry_scheduler: Arc::new(RetryScheduler::from_config(config)),
        }
    }

    /// Returns the HS IDs relevant for this run, ordered by their priority.
    async fn hs_by_priority(&self) -> Result<Vec<String>, DynError> {
        let active_hs_ids = Homeserver::get_all_active_from_graph().await?;

        let result_hs_ids: Vec<String> = active_hs_ids
            .into_iter()
            // Exclude the primary HS, as it is processed separately
            .filter(|hs_id| hs_id != self.primary_homeserver.as_ref())
            // Exclude any blacklisted HS
            .filter(|hs_id| !self.hs_blacklist.is_blacklisted(hs_id))
            .collect();

        Ok(result_hs_ids)
    }
}

#[async_trait::async_trait]
impl TEventProcessorRunner for KeyBasedEventProcessorRunner {
    fn shutdown_rx(&self) -> Receiver<bool> {
        self.shutdown_rx.clone()
    }

    async fn build(&self, hs_id: &str) -> Result<Arc<dyn TEventProcessor>, DynError> {
        let homeserver_id = PubkyId::try_from(hs_id)?;

        Ok(Arc::new(KeyBasedEventProcessor {
            homeserver_id,
            limit: self.limit,
            event_handler: self.event_handler.clone(),
            event_source: self.event_source.clone(),
            user_not_found_backoff: self.user_not_found_backoff.clone(),
            retry_scheduler: self.retry_scheduler.clone(),
            hs_blacklist: self.hs_blacklist.clone(),
            shutdown_rx: self.shutdown_rx.clone(),
        }))
    }

    async fn pre_run(&self) -> Result<Vec<String>, DynError> {
        let mut hs_ids = self.hs_by_priority().await?;
        hs_ids.truncate(self.monitored_hs_limit);

        // Recorded on every run, including an empty one, so the saturation ratio
        // always has a denominator in force.
        EXTERNAL_HS_METRICS.record_run(self.monitored_hs_limit, hs_ids.len());

        Ok(hs_ids)
    }

    async fn backoff_hs_should_skip(&self, hs_id: &str) -> bool {
        let backoff = self.backoff.lock().await;
        backoff.should_skip(hs_id)
    }

    async fn backoff_hs_record_result(&self, hs_id: &str, status: &ProcessorRunStatus) {
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
            debug!(homeserver = %hs_id, ?duration, ?status, "Event processor run completed");
        }

        let count_ok = stats.count_ok();
        let count_error = stats.count_error();
        let count_panic = stats.count_panic();
        let count_timeout = stats.count_timeout();
        let count_failed_to_build = stats.count_failed_to_build();
        let count_skipped = stats.count_skipped();
        let had_issues = count_error + count_panic + count_timeout + count_failed_to_build > 0;

        if had_issues {
            warn!(
                hs_ok = count_ok,
                hs_skipped = count_skipped,
                hs_failed_to_build = count_failed_to_build,
                hs_error = count_error,
                hs_panic = count_panic,
                hs_timeout = count_timeout,
                "Key-based indexing finished with issues"
            );
        } else if count_skipped > 0 {
            warn!(
                hs_ok = count_ok,
                hs_skipped = count_skipped,
                "Key-based indexing finished; some homeservers skipped (backoff)"
            );
        } else if count_ok == 0 {
            info!("Key-based indexing finished: no external homeservers");
        } else {
            info!(hs_ok = count_ok, "Key-based indexing finished");
        }

        ProcessedStats(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::ExternalHsMetrics;
    use opentelemetry::metrics::MeterProvider;
    use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData, ResourceMetrics};
    use opentelemetry_sdk::metrics::{InMemoryMetricExporter, SdkMeterProvider};

    /// Builds the metrics against an in-memory exporter.
    fn metered_metrics() -> (ExternalHsMetrics, SdkMeterProvider, InMemoryMetricExporter) {
        let exporter = InMemoryMetricExporter::default();
        let provider = SdkMeterProvider::builder()
            .with_periodic_exporter(exporter.clone())
            .build();
        let metrics = ExternalHsMetrics::with_meter(provider.meter("test"));
        (metrics, provider, exporter)
    }

    /// Reads the last data point of a u64 gauge, `None` when the instrument was
    /// not exported.
    fn gauge_value(metrics: &[ResourceMetrics], name: &str) -> Option<u64> {
        for resource_metrics in metrics {
            for scope_metrics in resource_metrics.scope_metrics() {
                for metric in scope_metrics.metrics().filter(|m| m.name() == name) {
                    let AggregatedMetrics::U64(MetricData::Gauge(gauge)) = metric.data() else {
                        continue;
                    };
                    let mut value = None;
                    for data_point in gauge.data_points() {
                        value = Some(data_point.value());
                    }
                    return value;
                }
            }
        }
        None
    }

    #[test]
    fn records_the_limit_and_the_selected_homeservers() {
        let (metrics, provider, exporter) = metered_metrics();

        metrics.record_run(50, 47);
        provider.force_flush().unwrap();
        let exported = exporter.get_finished_metrics().unwrap();

        assert_eq!(
            gauge_value(&exported, "watcher.external_hs.monitored_limit"),
            Some(50),
            "the configured cap must be exported, it is the ratio's denominator"
        );
        assert_eq!(
            gauge_value(&exported, "watcher.external_hs.indexed"),
            Some(47),
            "the homeservers the run selected must be exported"
        );
    }
}
