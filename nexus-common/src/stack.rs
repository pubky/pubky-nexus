use std::time::Duration;

use crate::db::setup::setup_graph;
use crate::db::Neo4JConfig;
use crate::db::{Neo4jConnector, RedisConnector, NEO4J_CONNECTOR, REDIS_CONNECTOR};
use crate::{Config as StackConfig, Level};
use opentelemetry::{global, KeyValue};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, MetricExporter, SpanExporter, WithExportConfig};
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use tracing::{debug, error, info};
use tracing_subscriber::{fmt, EnvFilter, Layer};
use tracing_subscriber::{layer::SubscriberExt, Registry};

pub struct StackManager {}

impl StackManager {
    pub async fn setup(name: &str, config: &StackConfig) {
        // Initialize logging and metrics
        Self::setup_logging(name, &config.otlp_endpoint, config.log_level).await;
        Self::setup_metrics(name, &config.otlp_endpoint).await;

        // Initialize Redis and Neo4j
        Self::setup_redis(&config.db.redis).await;
        Self::setup_neo4j(&config.db.neo4j).await;
    }

    pub async fn setup_redis(redis_uri: &str) {
        let redis_connector = RedisConnector::new_connection(redis_uri)
            .await
            .expect("Failed to connect to Redis");

        match REDIS_CONNECTOR.set(redis_connector) {
            Err(e) => debug!("RedisConnector was already set: {:?}", e),
            Ok(()) => info!("RedisConnector successfully set up on {}", redis_uri),
        }
    }

    async fn setup_neo4j(neo4j_config: &Neo4JConfig) {
        let neo4j_connector = Neo4jConnector::new_connection(
            &neo4j_config.uri,
            &neo4j_config.user,
            &neo4j_config.password,
        )
        .await
        .expect("Failed to connect to Neo4j");

        match NEO4J_CONNECTOR.set(neo4j_connector) {
            Err(e) => debug!("Neo4jConnector was already set: {:?}", e),
            Ok(()) => info!("Neo4jConnector successfully set up on {}", neo4j_config.uri),
        }

        // Set Neo4J graph data constraints
        setup_graph().await.unwrap_or_default();
    }

    async fn setup_logging(service_name: &str, otel_endpoint: &Option<String>, log_level: Level) {
        match otel_endpoint {
            None => Self::setup_local_logging(log_level),
            Some(endpoint) => {
                match Self::setup_otlp_logging(service_name, endpoint, log_level).await {
                    Ok(()) => info!("OpenTelemetry Logging initialized"),
                    Err(e) => error!("Failed to initialize OpenTelemetry Logging: {:?}", e),
                }
            }
        }
    }

    fn setup_local_logging(log_level: Level) {
        let subscriber = fmt::Subscriber::builder()
            .compact()
            .with_env_filter(
                EnvFilter::new(log_level.as_str()).add_directive("mainline=info".parse().unwrap()),
            )
            .with_line_number(true)
            .finish();

        match tracing::subscriber::set_global_default(subscriber) {
            Ok(()) => info!("Local application logging initialized"),
            Err(e) => debug!("Logging already initialized: {:?}", e),
        }
    }

    async fn setup_otlp_logging(
        service_name: &str,
        otel_endpoint: &String,
        log_level: Level,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Set up OpenTelemetry Tracer (Spans)
        let tracing_exporter = SpanExporter::builder()
            .with_tonic()
            .with_endpoint(otel_endpoint.clone())
            .with_timeout(Duration::from_secs(3))
            .build()
            .map_err(|e| format!("OTLP Tracing Exporter Error: {}", e))?;

        // Collects spans in memory and sends them in batches
        let tracer_provider = SdkTracerProvider::builder()
            .with_resource(Self::create_resource(service_name))
            .with_batch_exporter(tracing_exporter)
            .build();

        // Registers OpenTelemetry as the global tracing provider
        // Ensures that all spans created in the app are processed and exported to an OTLP backend (signoz or jaeger)
        global::set_tracer_provider(tracer_provider.clone());

        // Set up OpenTelemetry Logging
        let logging_exporter = LogExporter::builder()
            .with_tonic()
            .with_endpoint(otel_endpoint.clone())
            .with_timeout(Duration::from_secs(3))
            .build()
            .map_err(|e| format!("OTLP Logging Exporter Error: {}", e))?;

        let logging_provider = SdkLoggerProvider::builder()
            .with_resource(Self::create_resource(service_name))
            .with_batch_exporter(logging_exporter)
            .build();

        // Apply log filters for verbosity control
        // This ensures only relevant logs are sent to OpenTelemetry, reducing unnecessary data transmission
        let otlp_layer = OpenTelemetryTracingBridge::new(&logging_provider).with_filter(
            EnvFilter::new(log_level.as_str())
                .add_directive("opentelemetry=error".parse().unwrap())
                .add_directive("h2=error".parse().unwrap())
                .add_directive("tower=info".parse().unwrap())
                .add_directive("mainline=info".parse().unwrap()),
        );

        // Configure the stdout logging layer
        let stdout_layer = fmt::layer().compact().with_line_number(true).with_filter(
            EnvFilter::new(log_level.as_str())
                .add_directive("opentelemetry=error".parse().unwrap())
                .add_directive("h2=error".parse().unwrap())
                .add_directive("tower=info".parse().unwrap())
                .add_directive("mainline=info".parse().unwrap()),
        );

        // Creates a tracing subscriber
        let subscriber = Registry::default().with(stdout_layer).with(otlp_layer);

        // Registers a global tracing subscriber that captures logs
        if tracing::subscriber::set_global_default(subscriber).is_ok() {
            info!(
                "OpenTelemetry Logging initialized (OTLP_ENDPOINT={})",
                otel_endpoint
            );
        } else {
            error!("Failed to initialize OpenTelemetry Logging: Already set globally!");
        }

        Ok(())
    }

    async fn setup_metrics(service_name: &str, otel_endpoint: &Option<String>) {
        match otel_endpoint {
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
                    .with_interval(std::time::Duration::from_secs(30))
                    .build();

                // Createa Meter Provider, which is responsible for managing and exporting metrics
                let provider = SdkMeterProvider::builder()
                    .with_resource(Self::create_resource(service_name))
                    .with_reader(reader)
                    .build();

                // Register globally the metrics
                global::set_meter_provider(provider);

                info!("Metrics initialized (OTLP_ENDPOINT={})", endpoint);
            }
        }
    }

    fn create_resource(service_name: &str) -> Resource {
        Resource::builder_empty()
            .with_attribute(KeyValue::new("service.name", String::from(service_name)))
            .build()
    }
}
