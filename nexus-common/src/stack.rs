use crate::config::OtlpConfig;
use crate::db::kv::search::set_ft_search_timeout_ms;
use crate::db::{fetch_key_from_graph, fetch_row_from_graph, queries};
use crate::db::{Neo4jConnector, RedisConnector};
use crate::types::DynError;
use crate::{Level, ModeMismatch, NexusMode, StackConfig};
use opentelemetry::trace::TracerProvider;
use opentelemetry::{global, KeyValue};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, MetricExporter, SpanExporter, WithExportConfig};
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use std::time::Duration;
use tokio::sync::OnceCell;
use tracing::{error, info};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{fmt, EnvFilter, Layer};
use tracing_subscriber::{layer::SubscriberExt, Registry};

/// Stores the [`StackConfig`] used for the first initialization.
/// Subsequent calls to [`StackManager::setup`] verify the config matches.
static STACK_CONFIG: OnceCell<StackConfig> = OnceCell::const_new();

/// Manages one-time initialization of the shared infrastructure stack
/// (logging, metrics, database connections).
///
/// Each builder's `start()` method calls [`StackManager::setup`] internally.
pub struct StackManager;

impl StackManager {
    pub async fn setup(config: &StackConfig) -> Result<(), DynError> {
        let stored = STACK_CONFIG
            .get_or_try_init(|| async {
                Self::setup_logging(&config.otlp, config.log_level).await;
                Self::setup_metrics(&config.otlp).await;

                RedisConnector::init(&config.db.redis).await?;
                Neo4jConnector::init(&config.db.neo4j).await?;
                set_ft_search_timeout_ms(config.db.ft_search_timeout_ms);
                Ok::<_, DynError>(config.clone())
            })
            .await?;

        if stored != config {
            return Err("StackManager already initialized with a different StackConfig".into());
        }

        Ok(())
    }

    /// The mode this process was set up with. [`NexusMode::Full`] before
    /// [`StackManager::setup`] has run.
    pub fn mode() -> NexusMode {
        STACK_CONFIG
            .get()
            .map(|config| config.mode)
            .unwrap_or_default()
    }

    /// Checks that the database was indexed in the configured mode, and locks an empty
    /// database to it. Run by the API and the watcher at start, after [`Self::setup`].
    ///
    /// The lock is a `(:NexusMode)` node in the graph, so `nexusd db clear` removes it with
    /// everything else. A graph that holds users but no lock predates it and is treated as
    /// full. Light and full data must never mix, so a mismatch is an error, not a warning.
    pub async fn ensure_mode_lock() -> Result<NexusMode, DynError> {
        let configured = Self::mode();
        let row = fetch_row_from_graph(queries::get::get_mode_lock())
            .await?
            .ok_or("mode lock query returned no row")?;
        let stored = match row.get::<Option<String>>("mode")? {
            Some(value) => Some(
                NexusMode::from_stored(&value)
                    .ok_or_else(|| format!("unknown mode in the mode lock: {value}"))?,
            ),
            None => None,
        };
        let has_data: bool = row.get("has_data")?;

        let locked = NexusMode::resolve_lock(stored, has_data, configured)?;
        if stored.is_none() {
            // Another process may lock the database between the read and this write. The
            // `uniqueNexusMode` constraint makes concurrent writes merge into one node, and
            // the write returns that node's mode, the first writer's, so check it again.
            let written: String =
                fetch_key_from_graph(queries::put::set_mode_lock(locked.as_str()), "mode")
                    .await?
                    .ok_or("mode lock write returned no row")?;
            let written = NexusMode::from_stored(&written)
                .ok_or_else(|| format!("unknown mode in the mode lock: {written}"))?;
            if written != configured {
                return Err(ModeMismatch {
                    locked: written,
                    configured,
                }
                .into());
            }
            info!("Locked the database to {locked} mode");
        }
        Ok(locked)
    }

    async fn setup_logging(otlp: &OtlpConfig, log_level: Level) {
        match &otlp.endpoint {
            None => Self::setup_local_logging(log_level),
            Some(_) => match Self::setup_otlp_logging(otlp, log_level).await {
                Ok(()) => info!(
                    "OpenTelemetry Logging initialized for {} service",
                    otlp.name
                ),
                Err(e) => error!("Failed to initialize OpenTelemetry Logging: {:?}", e),
            },
        }
    }

    /// Builds an [`EnvFilter`] at the given level with directives to suppress noisy dependencies.
    ///
    /// HTTP/TLS crates are always capped. Discovery crates (`mainline`, `pkarr`, `pubky`) are
    /// capped unless the global level is [`Level::Trace`], so their detail only appears when
    /// explicitly tracing.
    fn env_filter(log_level: Level) -> EnvFilter {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            let mut filter = EnvFilter::new(log_level.as_str())
                .add_directive("opentelemetry=error".parse().unwrap())
                .add_directive("h2=error".parse().unwrap())
                .add_directive("tower=info".parse().unwrap())
                .add_directive("reqwest=warn".parse().unwrap())
                .add_directive("hyper_util=warn".parse().unwrap())
                .add_directive("rustls=warn".parse().unwrap());

            if log_level != Level::Trace {
                for directive in ["mainline=info", "pkarr=warn", "pubky=warn"] {
                    filter = filter.add_directive(directive.parse().unwrap());
                }
            }

            filter
        })
    }

    fn setup_local_logging(log_level: Level) {
        // Enable log-to-tracing bridge so that `log`-based crates (e.g., neo4rs) emit through our `tracing` subscriber
        let _ = tracing_log::LogTracer::init();

        // Build an env‐based filter
        let env_filter = Self::env_filter(log_level);

        // Create a formatting layer
        let fmt_layer = fmt::layer().compact().with_line_number(true);

        // Compose the subscriber
        let subscriber = Registry::default().with(env_filter).with(fmt_layer);

        if tracing::subscriber::set_global_default(subscriber).is_ok() {
            tracing::info!("Local application logging initialized");
        }
    }

    async fn setup_otlp_logging(
        otlp: &OtlpConfig,
        log_level: Level,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let otel_endpoint = otlp
            .endpoint
            .as_deref()
            .expect("OTLP endpoint must be set in [stack.otlp]");

        // TODO: Add local tracer, https://github.com/pubky/pubky-nexus/issues/356
        // Set up OpenTelemetry Tracer (Spans)
        let tracing_exporter = SpanExporter::builder()
            .with_tonic()
            .with_endpoint(otel_endpoint.to_string())
            .with_timeout(Duration::from_secs(3))
            .build()
            .map_err(|e| format!("OTLP Tracing Exporter Error: {e}"))?;

        // Collects spans in memory and sends them in batches
        let tracer_provider = SdkTracerProvider::builder()
            .with_resource(Self::create_resource(otlp))
            .with_batch_exporter(tracing_exporter)
            .build();

        // Registers OpenTelemetry as the global tracing provider
        // Ensures that all spans created in the app are processed and exported to an OTLP backend (signoz or jaeger)
        global::set_tracer_provider(tracer_provider.clone());

        // Set up OpenTelemetry Logging
        let logging_exporter = LogExporter::builder()
            .with_tonic()
            .with_endpoint(otel_endpoint.to_string())
            .with_timeout(Duration::from_secs(3))
            .build()
            .map_err(|e| format!("OTLP Logging Exporter Error: {e}"))?;

        let logging_provider = SdkLoggerProvider::builder()
            .with_resource(Self::create_resource(otlp))
            .with_batch_exporter(logging_exporter)
            .build();

        // Apply log filters for verbosity control
        // This ensures only relevant logs are sent to OpenTelemetry, reducing unnecessary data transmission
        let otlp_layer = OpenTelemetryTracingBridge::new(&logging_provider)
            .with_filter(Self::env_filter(log_level));

        // Configure the stdout logging layer
        let stdout_layer = fmt::layer()
            .compact()
            .with_line_number(true)
            .with_filter(Self::env_filter(log_level));

        // Bridge tracing spans into OpenTelemetry trace spans.
        // This allows #[instrument] and info_span!() to produce OTel spans
        // that are exported alongside manually-created OTel spans.
        let otel_trace_layer = OpenTelemetryLayer::new(tracer_provider.tracer(otlp.name.clone()))
            .with_filter(Self::env_filter(log_level));

        // Creates a tracing subscriber
        let subscriber = Registry::default()
            .with(stdout_layer)
            .with(otlp_layer)
            .with(otel_trace_layer);

        // Registers a global tracing subscriber that captures logs
        if tracing::subscriber::set_global_default(subscriber).is_ok() {
            info!(
                "OpenTelemetry endpoint listening on (OTLP_ENDPOINT={})",
                otel_endpoint
            );
        } else {
            error!("Failed to initialize OpenTelemetry Logging: Already set globally!");
        }

        Ok(())
    }

    async fn setup_metrics(otlp: &OtlpConfig) {
        match &otlp.endpoint {
            None => info!("Metrics collection is disabled. No metrics will be exported"),
            Some(endpoint) => {
                // Configure the exporter to collect and send metrics to an OTLP
                let metric_exporter = MetricExporter::builder()
                    .with_tonic()
                    .with_endpoint(endpoint.clone())
                    .with_timeout(Duration::from_secs(3))
                    .build()
                    .expect("Failed to create OTLP metric exporter");

                // Create a periodic metrics reader that collects and exports metrics at a fixed interval
                let reader = PeriodicReader::builder(metric_exporter)
                    .with_interval(Duration::from_secs(30))
                    .build();

                // Createa Meter Provider, which is responsible for managing and exporting metrics
                let provider = SdkMeterProvider::builder()
                    .with_resource(Self::create_resource(otlp))
                    .with_reader(reader)
                    .build();

                // Register globally the metrics
                global::set_meter_provider(provider);
                info!(
                    "OpenTelemetry Metrics initialized for {} service",
                    otlp.name
                );
            }
        }
    }

    /// OTEL resource: `resource_attributes`, then `service.name` from `otlp.name` (wins on conflict).
    fn create_resource(otlp: &OtlpConfig) -> Resource {
        Resource::builder_empty()
            .with_attributes(
                otlp.resource_attributes
                    .iter()
                    .map(|(key, value)| KeyValue::new(key.clone(), value.clone())),
            )
            .with_service_name(otlp.name.clone())
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::{Key, Value};
    use std::collections::HashMap;

    #[test]
    fn create_resource_applies_attributes_and_service_name() {
        let otlp = OtlpConfig {
            name: "nexusd".into(),
            endpoint: None,
            resource_attributes: HashMap::from([
                ("host".into(), "nexus-1".into()),
                ("env".into(), "prod".into()),
            ]),
        };
        let resource = StackManager::create_resource(&otlp);

        assert_eq!(
            resource.get(&Key::new("service.name")),
            Some(Value::from("nexusd"))
        );
        assert_eq!(
            resource.get(&Key::new("host")),
            Some(Value::from("nexus-1"))
        );
        assert_eq!(resource.get(&Key::new("env")), Some(Value::from("prod")));
    }

    #[test]
    fn create_resource_service_name_wins_over_attribute_map() {
        let otlp = OtlpConfig {
            name: "from-config".into(),
            endpoint: None,
            resource_attributes: HashMap::from([("service.name".into(), "from-map".into())]),
        };
        let resource = StackManager::create_resource(&otlp);

        assert_eq!(
            resource.get(&Key::new("service.name")),
            Some(Value::from("from-config"))
        );
    }
}

#[cfg(test)]
mod mode_lock_tests {
    use super::StackManager;
    use crate::db::graph::setup::setup_mode_lock_constraint;
    use crate::db::{exec_single_row, fetch_all_rows_from_graph, graph::Query};
    use crate::types::DynError;
    use crate::{NexusMode, StackConfig};

    /// The lock nodes, oldest first, as `(mode, created_at)`.
    async fn lock_nodes() -> Result<Vec<(String, i64)>, DynError> {
        let rows = fetch_all_rows_from_graph(Query::new(
            "test_read_mode_locks",
            "MATCH (lock:NexusMode) RETURN lock.mode AS mode, lock.created_at AS created_at
             ORDER BY created_at",
        ))
        .await?;
        rows.iter()
            .map(|row| Ok((row.get("mode")?, row.get("created_at")?)))
            .collect()
    }

    async fn drop_locks() -> Result<(), DynError> {
        exec_single_row(Query::new(
            "test_drop_mode_locks",
            "MATCH (lock:NexusMode) DETACH DELETE lock",
        ))
        .await?;
        Ok(())
    }

    /// One test, run in order, because every step reads and writes the same lock node.
    #[tokio_shared_rt::test(shared)]
    async fn test_mode_lock() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        // Duplicates written before the constraint existed are reduced to the oldest, and
        // the constraint is then created over the survivor.
        exec_single_row(Query::new(
            "test_drop_mode_lock_constraint",
            "DROP CONSTRAINT uniqueNexusMode IF EXISTS",
        ))
        .await?;
        drop_locks().await?;
        exec_single_row(Query::new(
            "test_duplicate_mode_locks",
            "UNWIND [3, 1, 2] AS created_at
             CREATE (:NexusMode {id: 'mode', mode: 'full', created_at: created_at})",
        ))
        .await?;
        setup_mode_lock_constraint().await?;
        assert_eq!(lock_nodes().await?, vec![("full".to_string(), 1)]);
        let duplicate = exec_single_row(Query::new(
            "test_duplicate_after_constraint",
            "CREATE (:NexusMode {id: 'mode', mode: 'full'})",
        ))
        .await;
        assert!(duplicate.is_err(), "the constraint refuses a second lock");

        // Concurrent starts against the mock graph (users, no lock) write one lock, and all
        // of them agree on it: the graph predates the lock, so it is full.
        drop_locks().await?;
        let starts: Vec<_> = (0..8)
            .map(|_| tokio::spawn(StackManager::ensure_mode_lock()))
            .collect();
        for start in starts {
            let locked = start.await?.map_err(|e| e.to_string())?;
            assert_eq!(locked, NexusMode::Full);
        }
        let locks = lock_nodes().await?;
        assert_eq!(locks.len(), 1, "one lock node: {locks:?}");
        assert_eq!(locks[0].0, "full");

        // Idempotent once locked.
        assert_eq!(StackManager::ensure_mode_lock().await?, NexusMode::Full);
        assert_eq!(lock_nodes().await?.len(), 1);

        drop_locks().await?;
        Ok(())
    }
}
