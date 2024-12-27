use std::time::Duration;

use crate::config::Config;
use crate::db::connectors::{
    neo4j::{Neo4jConnector, NEO4J_CONNECTOR},
    redis::{RedisConnector, REDIS_CONNECTOR},
};
use crate::db::graph::setup::setup_graph;
use opentelemetry::{global, KeyValue};
use opentelemetry_appender_tracing::layer;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::logs::LoggerProvider;
use opentelemetry_sdk::trace::TracerProvider;
use opentelemetry_sdk::Resource;
use tracing::{debug, info};
use tracing_subscriber::{layer::SubscriberExt, Registry};
use tracing_subscriber::{EnvFilter, Layer};

async fn setup_redis(config: &Config) {
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
        tracing_subscriber::fmt().pretty().init();
        return;
    }

    let tracing_exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(config.otlp_endpoint.clone())
        .with_timeout(Duration::from_secs(3))
        .build()
        .expect("Failed to create OTLP tracing exporter");

    let tracer_provider = TracerProvider::builder()
        .with_batch_exporter(tracing_exporter, opentelemetry_sdk::runtime::Tokio)
        .with_resource(Resource::new(vec![KeyValue::new("service.name", "nexus")]))
        .build();

    global::set_tracer_provider(tracer_provider.clone());

    let logging_exporter = opentelemetry_otlp::LogExporter::builder()
        .with_tonic()
        .with_endpoint(config.otlp_endpoint.clone())
        .with_timeout(Duration::from_secs(3))
        .build()
        .expect("Failed to create OTLP logging exporter");

    let logging_provider = LoggerProvider::builder()
        .with_batch_exporter(logging_exporter, opentelemetry_sdk::runtime::Tokio)
        .with_resource(Resource::new(vec![KeyValue::new("service.name", "nexus")]))
        .build();

    let otlp_layer = layer::OpenTelemetryTracingBridge::new(&logging_provider).with_filter(
        EnvFilter::from_default_env()
            .add_directive("opentelemetry=error".parse().unwrap())
            .add_directive("h2=error".parse().unwrap())
            .add_directive("tower=info".parse().unwrap()),
    );

    let subscriber = Registry::default()
        .with(
            tracing_subscriber::fmt::layer().pretty().with_filter(
                EnvFilter::from_default_env()
                    .add_directive("opentelemetry=error".parse().unwrap())
                    .add_directive("h2=error".parse().unwrap())
                    .add_directive("tower=info".parse().unwrap()),
            ),
        )
        .with(otlp_layer);

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");
}

async fn setup_metrics(config: &Config) {
    if config.otlp_endpoint.is_empty() {
        return;
    }
    let metric_exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_tonic()
        .with_endpoint(config.otlp_endpoint.clone())
        .with_timeout(Duration::from_secs(3))
        .build()
        .expect("Failed to create OTLP metric exporter");

    let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(
        metric_exporter,
        opentelemetry_sdk::runtime::Tokio,
    )
    .with_interval(std::time::Duration::from_secs(30))
    .with_timeout(Duration::from_secs(3))
    .build();

    let provider = opentelemetry_sdk::metrics::SdkMeterProvider::builder()
        .with_reader(reader)
        .with_resource(Resource::new(vec![KeyValue::new("service.name", "nexus")]))
        .build();
    global::set_meter_provider(provider);
}

pub async fn setup(config: &Config) {
    // Initialize logging and metrics
    setup_logging(config).await;
    setup_metrics(config).await;

    // Initialize Redis and Neo4j
    setup_redis(config).await;
    setup_neo4j(config).await;
}
