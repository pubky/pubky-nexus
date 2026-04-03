use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::{Stream, StreamExt};
use neo4rs::Row;
use opentelemetry::metrics::{Counter, Histogram};
use opentelemetry::trace::{Span, SpanKind, Status, TraceContextExt, Tracer};
use opentelemetry::{global, Context as OtelContext, KeyValue};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use tracing::warn;

use super::ops::{Graph, GraphOps};
use super::query::Query;

/// The OpenTelemetry meter name used by all Neo4j graph metrics.
///
/// Instruments created under this meter are exported via the global
/// `SdkMeterProvider` configured in [`crate::stack::StackManager::setup_metrics`].
const METER_NAME: &str = "neo4j";
const TRACER_NAME: &str = "nexus.neo4j";

/// Shared OpenTelemetry metric instruments for Neo4j query monitoring.
///
/// Created once per [`InstrumentedGraph`] instance and cloned into each
/// [`InstrumentedStream`]. All instruments are safe to clone (internally Arc'd).
#[derive(Clone)]
struct GraphMetrics {
    /// Total wall-clock duration of a query (execute + stream consumption).
    duration: Histogram<f64>,
    /// Duration of the Bolt RUN phase only (pool acquire + planning).
    execute_duration: Histogram<f64>,
    /// Number of rows returned by a query.
    rows: Histogram<u64>,
    /// Incremented on every failed `execute()` or `run()` call.
    errors: Counter<u64>,
    /// Incremented when a query's total duration exceeds the slow-query threshold.
    slow: Counter<u64>,
}

/// Returns the single-element attribute slice used for all Neo4j metric recordings.
fn query_attrs(label: Option<&'static str>) -> [KeyValue; 1] {
    [KeyValue::new("query", label.unwrap_or("unknown"))]
}

impl GraphMetrics {
    /// Create all instruments from the global OpenTelemetry meter provider.
    ///
    /// This should be called once per [`InstrumentedGraph`] construction. The
    /// instruments are no-ops if no meter provider has been registered
    /// (i.e. when OTLP is not configured), so there is zero overhead in
    /// that case.
    fn new() -> Self {
        let meter = global::meter(METER_NAME);
        Self {
            duration: meter
                .f64_histogram("neo4j.query.duration")
                .with_description(
                    "Total wall-clock time for a Neo4j query (execute + fetch), in milliseconds",
                )
                .with_unit("ms")
                .build(),
            execute_duration: meter
                .f64_histogram("neo4j.query.execute_duration")
                .with_description(
                    "Time spent in the Bolt RUN phase (pool acquire + query planning), in milliseconds",
                )
                .with_unit("ms")
                .build(),
            rows: meter
                .u64_histogram("neo4j.query.rows")
                .with_description("Number of rows returned per Neo4j query")
                .with_unit("{row}")
                .build(),
            errors: meter
                .u64_counter("neo4j.query.errors")
                .with_description("Total number of failed Neo4j query executions")
                .build(),
            slow: meter
                .u64_counter("neo4j.query.slow")
                .with_description(
                    "Total number of Neo4j queries exceeding the slow-query threshold",
                )
                .build(),
        }
    }
}

/// A stream wrapper that measures total query time, logs slow queries, and
/// records OpenTelemetry metrics when dropped.
struct InstrumentedStream {
    inner: BoxStream<'static, Result<Row, neo4rs::Error>>,
    label: Option<&'static str>,
    /// Populated cypher text for debug logging (only set when `slow_query_logging_include_cypher` is enabled).
    cypher: Option<String>,
    /// Pool-acquire + Bolt RUN round-trip (query planning & start of execution).
    execute_duration: Duration,
    /// Wall-clock time from stream creation to drop (row fetching & consumption).
    stream_start: Instant,
    row_count: usize,
    error_message: Option<String>,
    threshold: Option<Duration>,
    metrics: GraphMetrics,
    /// OTel span context for this query. The span is ended in [`Drop`]
    /// so it covers execute + full stream consumption.
    otel_cx: Option<OtelContext>,
}

impl InstrumentedStream {
    fn new(
        inner: BoxStream<'static, Result<Row, neo4rs::Error>>,
        label: Option<&'static str>,
        cypher: Option<String>,
        execute_duration: Duration,
        threshold: Option<Duration>,
        metrics: GraphMetrics,
        otel_cx: Option<OtelContext>,
    ) -> Self {
        Self {
            inner,
            label,
            cypher,
            execute_duration,
            stream_start: Instant::now(),
            row_count: 0,
            error_message: None,
            threshold,
            metrics,
            otel_cx,
        }
    }
}

impl Stream for InstrumentedStream {
    type Item = Result<Row, neo4rs::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let result = Pin::new(&mut self.inner).poll_next(cx);
        if let Poll::Ready(Some(result)) = &result {
            match result {
                Ok(_) => self.row_count += 1,
                Err(e) => self.error_message = Some(e.to_string()),
            }
        }
        result
    }
}

impl Drop for InstrumentedStream {
    fn drop(&mut self) {
        let fetch_duration = self.stream_start.elapsed();
        let total = self.execute_duration + fetch_duration;

        let attrs: &[KeyValue] = &query_attrs(self.label);

        // Always record metrics (no-op when OTLP is not configured).
        self.metrics.duration.record(ms(total), attrs);
        self.metrics
            .execute_duration
            .record(ms(self.execute_duration), attrs);
        self.metrics.rows.record(self.row_count as u64, attrs);

        if self.error_message.is_some() {
            self.metrics.errors.add(1, attrs);
        }

        let is_slow = self.threshold.is_some_and(|threshold| total > threshold);

        if is_slow {
            self.metrics.slow.add(1, attrs);

            if let Some(label) = &self.label {
                warn!(
                    total_ms = total.as_millis(),
                    execute_ms = self.execute_duration.as_millis(),
                    fetch_ms = fetch_duration.as_millis(),
                    rows = self.row_count,
                    query = %label,
                    cypher = self.cypher.as_deref().unwrap_or(""),
                    "Slow Neo4j query"
                );
            }
        }

        // End the OTel span with query timing attributes
        if let Some(cx) = self.otel_cx.take() {
            let span = cx.span();
            span.set_attribute(KeyValue::new("db.neo4j.rows", self.row_count as i64));
            span.set_attribute(KeyValue::new("db.neo4j.duration_ms", ms(total)));
            span.set_attribute(KeyValue::new(
                "db.neo4j.execute_ms",
                ms(self.execute_duration),
            ));
            if is_slow {
                span.set_attribute(KeyValue::new("db.neo4j.slow", true));
            }
            if let Some(err) = &self.error_message {
                span.set_status(Status::error(err.clone()));
            }
            span.end();
        }
    }
}

/// Decorator around [`GraphOps`] that provides slow-query logging and
/// OpenTelemetry metrics for every Neo4j query.
///
/// Wrap a plain [`Graph`] with `InstrumentedGraph::new(graph)` to gain
/// automatic observability. When the global OTLP meter provider is not
/// configured, the metric instruments are no-ops with negligible overhead.
#[derive(Clone)]
pub struct InstrumentedGraph<G = Graph> {
    inner: G,
    slow_query_threshold: Option<Duration>,
    log_cypher: bool,
    metrics: GraphMetrics,
}

impl<G: GraphOps> InstrumentedGraph<G> {
    pub fn new(graph: G) -> Self {
        Self {
            inner: graph,
            slow_query_threshold: None,
            log_cypher: false,
            metrics: GraphMetrics::new(),
        }
    }

    pub fn with_slow_query_threshold(mut self, threshold: Option<Duration>) -> Self {
        self.slow_query_threshold = threshold;
        self
    }

    pub fn with_log_cypher(mut self, enabled: bool) -> Self {
        self.log_cypher = enabled;
        self
    }

    fn warn_if_slow(
        &self,
        elapsed: Duration,
        attrs: &[KeyValue],
        label: Option<&'static str>,
        cypher: Option<&str>,
        suffix: &'static str,
    ) {
        if let Some(threshold) = self.slow_query_threshold {
            if elapsed > threshold {
                self.metrics.slow.add(1, attrs);
                if let Some(lbl) = label {
                    warn!(
                        elapsed_ms = elapsed.as_millis(),
                        query = %lbl,
                        cypher = cypher.unwrap_or(""),
                        "Slow Neo4j query{}",
                        suffix
                    );
                }
            }
        }
    }
}

#[async_trait]
impl<G: GraphOps> GraphOps for InstrumentedGraph<G> {
    async fn execute(
        &self,
        query: Query,
    ) -> neo4rs::Result<BoxStream<'static, Result<Row, neo4rs::Error>>> {
        let label = query.label();
        let span_name = label.unwrap_or("neo4j.query");
        let cypher = self.log_cypher.then(|| query.to_cypher_populated());

        // Create an OTel span for this query (child of the current context).
        let tracer = global::tracer(TRACER_NAME);
        let mut span = tracer
            .span_builder(span_name)
            .with_kind(SpanKind::Client)
            .start(&tracer);
        span.set_attribute(KeyValue::new("db.system", "neo4j"));

        let start = Instant::now();
        let result = self.inner.execute(query).await;
        let execute_duration = start.elapsed();

        match result {
            Ok(stream) => {
                // Store the span context in InstrumentedStream; it will be ended on drop
                // after all rows are consumed.
                let otel_cx = Some(OtelContext::current_with_span(span));
                let instrumented = InstrumentedStream::new(
                    stream,
                    label,
                    cypher,
                    execute_duration,
                    self.slow_query_threshold,
                    self.metrics.clone(),
                    otel_cx,
                );
                Ok(instrumented.boxed())
            }
            Err(e) => {
                let attrs: &[KeyValue] = &query_attrs(label);
                self.metrics.duration.record(ms(execute_duration), attrs);
                self.metrics
                    .execute_duration
                    .record(ms(execute_duration), attrs);
                self.metrics.errors.add(1, attrs);
                span.set_status(Status::error(e.to_string()));
                span.end();
                self.warn_if_slow(
                    execute_duration,
                    attrs,
                    label,
                    cypher.as_deref(),
                    " (execute failed)",
                );

                Err(e)
            }
        }
    }

    async fn run(&self, query: Query) -> neo4rs::Result<()> {
        let label = query.label();
        let span_name = label.unwrap_or("neo4j.query");
        let cypher = self.log_cypher.then(|| query.to_cypher_populated());

        let tracer = global::tracer(TRACER_NAME);
        let mut span = tracer
            .span_builder(span_name)
            .with_kind(SpanKind::Client)
            .start(&tracer);
        span.set_attribute(KeyValue::new("db.system", "neo4j"));

        let start = Instant::now();
        let result = self.inner.run(query).await;
        let elapsed = start.elapsed();

        let attrs: &[KeyValue] = &query_attrs(label);
        self.metrics.duration.record(ms(elapsed), attrs);
        self.metrics.execute_duration.record(ms(elapsed), attrs);

        span.set_attribute(KeyValue::new("db.neo4j.duration_ms", ms(elapsed)));

        match &result {
            Ok(()) => self.warn_if_slow(elapsed, attrs, label, cypher.as_deref(), ""),
            Err(e) => {
                self.metrics.errors.add(1, attrs);
                span.set_status(Status::error(e.to_string()));
                self.warn_if_slow(elapsed, attrs, label, cypher.as_deref(), " (failed)");
            }
        }

        span.end();
        result
    }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::graph::GraphOps;
    use futures::stream;
    use neo4rs::{BoltList, BoltType};
    use tracing_test::traced_test;

    /// Create a dummy `Row` with a single string field.
    fn dummy_row() -> Row {
        let fields = BoltList::from(vec![BoltType::String("n".into())]);
        let data = BoltList::from(vec![BoltType::String("value".into())]);
        Row::new(fields, data)
    }

    fn make_instrumented_stream(
        inner: BoxStream<'static, Result<Row, neo4rs::Error>>,
        label: Option<&'static str>,
        threshold: Option<Duration>,
    ) -> InstrumentedStream {
        InstrumentedStream::new(
            inner,
            label,
            None,
            Duration::ZERO,
            threshold,
            GraphMetrics::new(),
            None,
        )
    }

    // ── InstrumentedStream row counting ───────────────────────────

    #[tokio::test]
    async fn counts_rows_from_inner_stream() {
        let rows = vec![Ok(dummy_row()), Ok(dummy_row()), Ok(dummy_row())];
        let inner = stream::iter(rows).boxed();
        let mut ts = make_instrumented_stream(inner, Some("test"), Some(Duration::from_secs(100)));

        while ts.next().await.is_some() {}
        assert_eq!(ts.row_count, 3);
    }

    #[tokio::test]
    async fn empty_stream_yields_none_and_zero_rows() {
        let inner = stream::empty().boxed();
        let mut ts = make_instrumented_stream(inner, Some("test"), Some(Duration::from_secs(100)));
        assert!(ts.next().await.is_none());
        assert_eq!(ts.row_count, 0);
    }

    // ── Slow-path doesn't abort the stream ─────────────────────────

    #[tokio::test]
    async fn slow_query_still_yields_all_rows() {
        // Threshold is 0ms — every query is "slow", but all rows must still be returned.
        let rows = vec![Ok(dummy_row()), Ok(dummy_row())];
        let inner = stream::iter(rows).boxed();
        let mut ts = make_instrumented_stream(inner, Some("slow_q"), Some(Duration::ZERO));

        let mut collected = Vec::new();
        while let Some(item) = ts.next().await {
            collected.push(item.expect("row should be Ok"));
        }
        assert_eq!(collected.len(), 2);
        assert_eq!(ts.row_count, 2);
    }

    // ── warn! emission ─────────────────────────────────────────────

    #[tokio::test]
    #[traced_test]
    async fn emits_warning_when_threshold_exceeded() {
        let inner = stream::iter(vec![Ok(dummy_row())]).boxed();
        // threshold = 0 → always slow
        let mut ts = make_instrumented_stream(inner, Some("slow_label"), Some(Duration::ZERO));
        while ts.next().await.is_some() {}
        drop(ts);

        assert!(logs_contain("Slow Neo4j query"));
        assert!(logs_contain("slow_label"));
    }

    #[tokio::test]
    #[traced_test]
    async fn no_warning_when_under_threshold() {
        let inner = stream::empty().boxed();
        let ts = make_instrumented_stream(inner, Some("fast_q"), Some(Duration::from_secs(600)));
        drop(ts);

        assert!(!logs_contain("Slow Neo4j query"));
    }

    #[tokio::test]
    #[traced_test]
    async fn no_warning_when_label_is_none() {
        let inner = stream::empty().boxed();
        // threshold = 0 but no label → drop skips logging entirely
        let ts = make_instrumented_stream(inner, None, Some(Duration::ZERO));
        drop(ts);

        assert!(!logs_contain("Slow Neo4j query"));
    }

    #[tokio::test]
    #[traced_test]
    async fn warning_includes_cypher_when_set() {
        let ts = InstrumentedStream::new(
            stream::empty().boxed(),
            Some("cypher_q"),
            Some("MATCH (n) RETURN n".into()),
            Duration::ZERO,
            Some(Duration::ZERO),
            GraphMetrics::new(),
            None,
        );
        drop(ts);

        assert!(logs_contain("MATCH (n) RETURN n"));
    }

    // ── InstrumentedGraph ──────────────────────────────────────────

    /// Mock `GraphOps` for testing `InstrumentedGraph` without a real Neo4j connection.
    #[derive(Clone)]
    struct MockGraph {
        row_count: usize,
    }

    #[async_trait]
    impl GraphOps for MockGraph {
        async fn execute(
            &self,
            _query: Query,
        ) -> neo4rs::Result<BoxStream<'static, Result<Row, neo4rs::Error>>> {
            let rows: Vec<Result<Row, neo4rs::Error>> =
                (0..self.row_count).map(|_| Ok(dummy_row())).collect();
            Ok(stream::iter(rows).boxed())
        }

        async fn run(&self, _query: Query) -> neo4rs::Result<()> {
            Ok(())
        }
    }

    fn test_query() -> Query {
        Query::new("test_label", "MATCH (n) RETURN n")
    }

    #[tokio::test]
    async fn instrumented_graph_execute_returns_all_rows() {
        let ig = InstrumentedGraph::new(MockGraph { row_count: 3 });
        let mut stream = ig.execute(test_query()).await.unwrap();

        let mut count = 0;
        while stream.next().await.is_some() {
            count += 1;
        }
        assert_eq!(count, 3);
    }

    #[tokio::test]
    #[traced_test]
    async fn instrumented_graph_run_warns_on_slow_query() {
        let ig = InstrumentedGraph::new(MockGraph { row_count: 0 })
            .with_slow_query_threshold(Some(Duration::ZERO));
        ig.run(test_query()).await.unwrap();

        assert!(logs_contain("Slow Neo4j query"));
        assert!(logs_contain("test_label"));
    }

    #[tokio::test]
    #[traced_test]
    async fn instrumented_graph_run_no_warning_under_threshold() {
        let ig = InstrumentedGraph::new(MockGraph { row_count: 0 })
            .with_slow_query_threshold(Some(Duration::from_secs(600)));
        ig.run(test_query()).await.unwrap();

        assert!(!logs_contain("Slow Neo4j query"));
    }
}
