use std::time::Duration;

use crate::db::graph::setup::setup_graph;
use crate::{
    db::connectors::{
        neo4j::{Neo4jConnector, NEO4J_CONNECTOR},
        redis::{RedisConnector, REDIS_CONNECTOR},
    },
    Config,
};
use opentelemetry::{global, KeyValue};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, MetricExporter, SpanExporter, WithExportConfig};
use opentelemetry_sdk::logs::LoggerProvider;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::trace::TracerProvider;
use opentelemetry_sdk::Resource;
use tracing::{debug, error, info};
use tracing_subscriber::{fmt, EnvFilter, Layer};
use tracing_subscriber::{layer::SubscriberExt, Registry};

pub struct StackManager {}

impl StackManager {
    pub async fn setup_redis(config: &Config) {
        let redis_connector = RedisConnector::new_connection(&config.redis_uri())
            .await
            .expect("Failed to connect to Redis");

        match REDIS_CONNECTOR.set(redis_connector) {
            Err(e) => debug!("RedisConnector was already set: {:?}", e),
            Ok(()) => info!("RedisConnector successfully set"),
        }
    }

    async fn setup_neo4j(config: &Config) {
        let neo4j_connector = Neo4jConnector::new_connection(
            &config.neo4j_uri(),
            &config.neo4j_username,
            &config.neo4j_password,
        )
        .await
        .expect("Failed to connect to Neo4j");

        match NEO4J_CONNECTOR.set(neo4j_connector) {
            Err(e) => debug!("Neo4jConnector was already set: {:?}", e),
            Ok(()) => info!("Neo4jConnector successfully set"),
        }

        // Set Neo4J graph data constraints
        setup_graph().await.unwrap_or_default();
    }

    async fn setup_logging(config: &Config) {
        if config.otlp_endpoint.is_empty() {
            Self::setup_local_logging();
            return;
        }

        match Self::setup_otlp_logging(config).await {
            Ok(()) => info!("OpenTelemetry Logging initialized"),
            Err(e) => error!("Failed to initialize OpenTelemetry Logging: {:?}", e),
        }
    }

    fn setup_local_logging() {
        let subscriber = fmt::Subscriber::builder()
            .compact()
            .with_env_filter(EnvFilter::from_default_env())
            .with_line_number(true)
            .finish();

        match tracing::subscriber::set_global_default(subscriber) {
            Ok(()) => info!("Local application logging initialized"),
            Err(e) => debug!("Logging already initialized: {:?}", e),
        }
    }

    async fn setup_otlp_logging(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
        let endpoint = &config.otlp_endpoint;

        // Set up OpenTelemetry Tracer (Spans)
        let tracing_exporter = SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint.clone())
            .with_timeout(Duration::from_secs(3))
            .build()
            .map_err(|e| format!("OTLP Tracing Exporter Error: {}", e))?;
        
        // TODO: That service name, has to came from each crate in the future
        let service_name = Resource::new(vec![KeyValue::new("service.name", "nexus.watcher")]);

        // Collects spans in memory and sends them in batches
        let tracer_provider = TracerProvider::builder()
            .with_resource(service_name.clone())
            .with_batch_exporter(tracing_exporter, opentelemetry_sdk::runtime::Tokio)
            .build();

        // Registers OpenTelemetry as the global tracing provider
        // Ensures that all spans created in the app are processed and exported to an OTLP backend (signoz or jaeger)
        global::set_tracer_provider(tracer_provider.clone());

        // Set up OpenTelemetry Logging
        let logging_exporter = LogExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint.clone())
            .with_timeout(Duration::from_secs(3))
            .build()
            .map_err(|e| format!("OTLP Logging Exporter Error: {}", e))?;

        let logging_provider = LoggerProvider::builder()
            .with_resource(service_name)
            .with_batch_exporter(logging_exporter, opentelemetry_sdk::runtime::Tokio)
            .build();

        // Apply log filters for verbosity control
        // This ensures only relevant logs are sent to OpenTelemetry, reducing unnecessary data transmission
        let otlp_layer = OpenTelemetryTracingBridge::new(&logging_provider).with_filter(
            EnvFilter::from_default_env()
                .add_directive("opentelemetry=error".parse().unwrap())
                .add_directive("h2=error".parse().unwrap())
                .add_directive("tower=info".parse().unwrap())
                .add_directive("mainline=info".parse().unwrap()),
        );

        // Configure the stdout logging layer
        let stdout_layer = fmt::layer().compact().with_line_number(true).with_filter(
            EnvFilter::from_default_env()
                .add_directive("opentelemetry=error".parse().unwrap())
                .add_directive("h2=error".parse().unwrap())
                .add_directive("tower=info".parse().unwrap())
                .add_directive("mainline=info".parse().unwrap())
        );
        
        // Creates a tracing subscriber
        let subscriber = Registry::default()
            .with(stdout_layer)
            .with(otlp_layer);

        // Registers a global tracing subscriber that captures logs
        if tracing::subscriber::set_global_default(subscriber).is_ok() {
            info!(
                "OpenTelemetry Logging initialized (OTLP_ENDPOINT={})",
                endpoint
            );
        } else {
            error!("Failed to initialize OpenTelemetry Logging: Already set globally!");
        }

        Ok(())
    }

    async fn setup_metrics(config: &Config) {
        if config.otlp_endpoint.is_empty() {
            info!("Metrics collection is disabled. No metrics will be exported.");
            return;
        }

        // TODO: That service name, has to came from each crate in the future
        let service_name = Resource::new(vec![KeyValue::new("service.name", "nexus.watcher")]);

        // Configure the exporter to collect and send metrics to an OTLP
        let metric_exporter = MetricExporter::builder()
            .with_tonic()
            .with_endpoint(config.otlp_endpoint.clone())
            .with_timeout(Duration::from_secs(3))
            .build()
            .expect("Failed to create OTLP metric exporter");

        // Create a periodic metrics reader that collects and exports metrics at a fixed interval
        let reader = PeriodicReader::builder(
            metric_exporter,
            opentelemetry_sdk::runtime::Tokio,
        )
            .with_interval(std::time::Duration::from_secs(30))
            .with_timeout(Duration::from_secs(3))
            .build();

        // Createa Meter Provider, which is responsible for managing and exporting metrics
        let provider = SdkMeterProvider::builder()
            .with_resource(service_name)
            .with_reader(reader)
            .build();

        // Register globally the metrics
        global::set_meter_provider(provider);

        info!(
            "Metrics initialized (OTLP_ENDPOINT={})",
            config.otlp_endpoint
        );
    }

    pub async fn setup(config: &Config) {
        // Initialize logging and metrics
        Self::setup_logging(config).await;
        Self::setup_metrics(config).await;

        // Initialize Redis and Neo4j
        Self::setup_redis(config).await;
        Self::setup_neo4j(config).await;
    }
}
