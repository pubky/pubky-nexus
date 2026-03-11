use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::{Stream, StreamExt, TryStreamExt};
use neo4rs::Row;
use opentelemetry::metrics::{Counter, Histogram};
use opentelemetry::{global, KeyValue};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use tracing::warn;

use super::query::Query;
use crate::db::config::DEFAULT_SLOW_QUERY_THRESHOLD_MS;

/// The OpenTelemetry meter name used by all Neo4j graph metrics.
///
/// Instruments created under this meter are exported via the global
/// `SdkMeterProvider` configured in [`crate::stack::StackManager::setup_metrics`].
const METER_NAME: &str = "neo4j";

/// Abstraction over graph database operations.
/// Callers depend on this trait, not the concrete implementations.
#[async_trait]
pub trait GraphOps: Send + Sync {
    /// Execute query, return boxed row stream.
    async fn execute(
        &self,
        query: Query,
    ) -> neo4rs::Result<BoxStream<'static, Result<Row, neo4rs::Error>>>;

    /// Fire-and-forget query execution.
    async fn run(&self, query: Query) -> neo4rs::Result<()>;
}

/// Thin wrapper around `neo4rs::Graph` implementing `GraphOps` without tracing.
#[derive(Clone)]
pub struct Graph {
    inner: neo4rs::Graph,
}

impl Graph {
    pub fn new(graph: neo4rs::Graph) -> Self {
        Self { inner: graph }
    }
}

#[async_trait]
impl GraphOps for Graph {
    async fn execute(
        &self,
        query: Query,
    ) -> neo4rs::Result<BoxStream<'static, Result<Row, neo4rs::Error>>> {
        let stream = self
            .inner
            .execute(query.into())
            .await?
            .into_stream()
            .map_err(Into::into)
            .boxed();
        Ok(stream)
    }

    async fn run(&self, query: Query) -> neo4rs::Result<()> {
        self.inner.run(query.into()).await
    }
}

/// Shared OpenTelemetry metric instruments for Neo4j query monitoring.
///
/// Created once per [`TracedGraph`] instance and cloned into each
/// [`TracedStream`]. All instruments are safe to clone (internally Arc'd).
///
/// # Emitted Metrics
///
/// | Metric name                   | Type      | Unit | Description |
/// |-------------------------------|-----------|------|-------------|
/// | `neo4j.query.duration`         | Histogram | s    | Total wall-clock time for a query (execute + fetch). Use percentiles (p50/p95/p99) to detect latency degradation over time. |
/// | `neo4j.query.execute_duration` | Histogram | s    | Time spent in the Bolt RUN phase (pool acquire + query planning + start of execution). A spike here with stable fetch times indicates connection-pool starvation or query-plan regression. |
/// | `neo4j.query.rows`            | Histogram | {row} | Number of rows returned per query. Detects cardinality explosions — a query that normally returns 10 rows suddenly returning 10k will show up here before latency spikes. |
/// | `neo4j.query.errors`          | Counter   | —    | Total number of failed query executions. Useful for error-rate alerting (rate > 0 sustained). |
/// | `neo4j.query.slow`            | Counter   | —    | Total number of queries exceeding the configured slow-query threshold. A rising rate signals degradation without needing to compute percentiles. |
///
/// All metrics carry a `query` attribute set to the query's static label
/// (e.g. `"get_user_by_id"`), enabling per-query-type filtering and grouping.
///
/// # Alerting Examples
///
/// | Alert                     | Metric                        | Condition                               |
/// |---------------------------|-------------------------------|-----------------------------------------|
/// | Latency degradation       | `neo4j.query.duration` p95    | > 2x rolling baseline                   |
/// | Cardinality explosion     | `neo4j.query.rows` p99        | sudden spike vs. historical range       |
/// | Error rate                | `neo4j.query.errors`          | rate > 0 sustained                      |
/// | Slow query storm          | `neo4j.query.slow`            | rate > N/min                            |
/// | Pool starvation           | `neo4j.query.execute_duration` p95 | spike while fetch time stays flat  |
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

impl GraphMetrics {
    /// Create all instruments from the global OpenTelemetry meter provider.
    ///
    /// This should be called once per [`TracedGraph`] construction. The
    /// instruments are no-ops if no meter provider has been registered
    /// (i.e. when OTLP is not configured), so there is zero overhead in
    /// that case.
    fn new() -> Self {
        let meter = global::meter(METER_NAME);
        Self {
            duration: meter
                .f64_histogram("neo4j.query.duration")
                .with_description(
                    "Total wall-clock time for a Neo4j query (execute + fetch), in seconds",
                )
                .with_unit("s")
                .build(),
            execute_duration: meter
                .f64_histogram("neo4j.query.execute_duration")
                .with_description(
                    "Time spent in the Bolt RUN phase (pool acquire + query planning), in seconds",
                )
                .with_unit("s")
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
///
/// Created by [`TracedGraph::execute`] to wrap the underlying row stream.
/// On drop it:
/// 1. Records `neo4j.query.duration`, `neo4j.query.execute_duration`, and
///    `neo4j.query.rows` histograms.
/// 2. Increments `neo4j.query.slow` counter if the total duration exceeds the
///    threshold.
/// 3. Emits a `tracing::warn` log for slow queries (with optional cypher text).
struct TracedStream {
    inner: BoxStream<'static, Result<Row, neo4rs::Error>>,
    label: Option<&'static str>,
    /// Populated cypher text for debug logging (only set when `slow_query_logging_include_cypher` is enabled).
    cypher: Option<String>,
    /// Pool-acquire + Bolt RUN round-trip (query planning & start of execution).
    execute_duration: Duration,
    /// Wall-clock time from stream creation to drop (row fetching & consumption).
    stream_start: Instant,
    row_count: usize,
    threshold: Duration,
    metrics: GraphMetrics,
}

impl Stream for TracedStream {
    type Item = Result<Row, neo4rs::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let result = Pin::new(&mut self.inner).poll_next(cx);
        match &result {
            Poll::Ready(Some(Ok(_))) => self.row_count += 1,
            Poll::Ready(Some(Err(_))) => {
                let attrs: &[KeyValue] = &[KeyValue::new("query", self.label.unwrap_or("unknown"))];
                self.metrics.errors.add(1, attrs);
            }
            _ => {}
        }
        result
    }
}

impl Drop for TracedStream {
    fn drop(&mut self) {
        let fetch_duration = self.stream_start.elapsed();
        let total = self.execute_duration + fetch_duration;

        let attrs: &[KeyValue] = &[KeyValue::new("query", self.label.unwrap_or("unknown"))];

        // Always record metrics (no-op when OTLP is not configured).
        self.metrics.duration.record(total.as_secs_f64(), attrs);
        self.metrics
            .execute_duration
            .record(self.execute_duration.as_secs_f64(), attrs);
        self.metrics.rows.record(self.row_count as u64, attrs);

        if total > self.threshold {
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
    }
}

/// Decorator around [`GraphOps`] that provides slow-query logging and
/// OpenTelemetry metrics for every Neo4j query.
///
/// Wrap a plain [`Graph`] with `TracedGraph::new(graph)` to gain
/// automatic observability. When the global OTLP meter provider is not
/// configured, the metric instruments are no-ops with negligible overhead.
///
/// # Configuration
///
/// | Method                         | Default   | Description |
/// |--------------------------------|-----------|-------------|
/// | [`with_slow_query_threshold`]  | 100 ms    | Duration above which a query is considered slow and triggers a `tracing::warn` log + `neo4j.query.slow` counter increment. |
/// | [`with_log_cypher`]            | `false`   | When `true`, the fully-populated Cypher string is included in slow-query log messages. Useful for debugging but can be verbose. |
///
/// # Metrics
///
/// See [`GraphMetrics`] for a full description of all emitted instruments,
/// their types, units, and recommended alerting strategies.
///
/// [`with_slow_query_threshold`]: TracedGraph::with_slow_query_threshold
/// [`with_log_cypher`]: TracedGraph::with_log_cypher
#[derive(Clone)]
pub struct TracedGraph<G = Graph> {
    inner: G,
    slow_query_threshold: Duration,
    log_cypher: bool,
    metrics: GraphMetrics,
}

impl<G: GraphOps> TracedGraph<G> {
    pub fn new(graph: G) -> Self {
        Self {
            inner: graph,
            slow_query_threshold: Duration::from_millis(DEFAULT_SLOW_QUERY_THRESHOLD_MS),
            log_cypher: false,
            metrics: GraphMetrics::new(),
        }
    }

    pub fn with_slow_query_threshold(mut self, threshold: Duration) -> Self {
        self.slow_query_threshold = threshold;
        self
    }

    pub fn with_log_cypher(mut self, enabled: bool) -> Self {
        self.log_cypher = enabled;
        self
    }
}

#[async_trait]
impl<G: GraphOps> GraphOps for TracedGraph<G> {
    async fn execute(
        &self,
        query: Query,
    ) -> neo4rs::Result<BoxStream<'static, Result<Row, neo4rs::Error>>> {
        let label = query.label();
        let cypher = if self.log_cypher {
            Some(query.to_cypher_populated())
        } else {
            None
        };

        let start = Instant::now();
        let result = self.inner.execute(query).await;
        let execute_duration = start.elapsed();

        match result {
            Ok(stream) => {
                let traced = TracedStream {
                    inner: stream,
                    label,
                    cypher,
                    execute_duration,
                    stream_start: Instant::now(), // Timestamp after execute(), so it only tracks the fetch phase,
                    row_count: 0,
                    threshold: self.slow_query_threshold,
                    metrics: self.metrics.clone(),
                };
                Ok(traced.boxed())
            }
            Err(e) => {
                let attrs: &[KeyValue] = &[KeyValue::new("query", label.unwrap_or("unknown"))];
                self.metrics
                    .duration
                    .record(execute_duration.as_secs_f64(), attrs);
                self.metrics
                    .execute_duration
                    .record(execute_duration.as_secs_f64(), attrs);
                self.metrics.errors.add(1, attrs);

                if execute_duration > self.slow_query_threshold {
                    self.metrics.slow.add(1, attrs);
                    if let Some(lbl) = &label {
                        warn!(
                            execute_ms = execute_duration.as_millis(),
                            query = %lbl,
                            cypher = cypher.as_deref().unwrap_or(""),
                            "Slow Neo4j query (execute failed)"
                        );
                    }
                }

                Err(e)
            }
        }
    }

    async fn run(&self, query: Query) -> neo4rs::Result<()> {
        let label = query.label();
        let cypher = if self.log_cypher {
            Some(query.to_cypher_populated())
        } else {
            None
        };

        let start = Instant::now();
        let result = self.inner.run(query).await;
        let elapsed = start.elapsed();

        let attrs: &[KeyValue] = &[KeyValue::new("query", label.unwrap_or("unknown"))];

        match &result {
            Ok(()) => {
                self.metrics.duration.record(elapsed.as_secs_f64(), attrs);

                if elapsed > self.slow_query_threshold {
                    self.metrics.slow.add(1, attrs);

                    if let Some(label) = &label {
                        warn!(
                            elapsed_ms = elapsed.as_millis(),
                            query = %label,
                            cypher = cypher.as_deref().unwrap_or(""),
                            "Slow Neo4j query"
                        );
                    }
                }
            }
            Err(_) => {
                self.metrics.duration.record(elapsed.as_secs_f64(), attrs);
                self.metrics.errors.add(1, attrs);

                if elapsed > self.slow_query_threshold {
                    self.metrics.slow.add(1, attrs);
                    if let Some(label) = &label {
                        warn!(
                            elapsed_ms = elapsed.as_millis(),
                            query = %label,
                            cypher = cypher.as_deref().unwrap_or(""),
                            "Slow Neo4j query (failed)"
                        );
                    }
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;
    use neo4rs::{BoltList, BoltType};
    use tracing_test::traced_test;

    /// Create a dummy `Row` with a single string field.
    fn dummy_row() -> Row {
        let fields = BoltList::from(vec![BoltType::String("n".into())]);
        let data = BoltList::from(vec![BoltType::String("value".into())]);
        Row::new(fields, data)
    }

    fn make_traced_stream(
        inner: BoxStream<'static, Result<Row, neo4rs::Error>>,
        label: Option<&'static str>,
        threshold: Duration,
    ) -> TracedStream {
        TracedStream {
            inner,
            label,
            cypher: None,
            execute_duration: Duration::ZERO,
            stream_start: Instant::now(),
            row_count: 0,
            threshold,
            metrics: GraphMetrics::new(),
        }
    }

    // ── TracedStream row counting ──────────────────────────────────

    #[tokio::test]
    async fn counts_rows_from_inner_stream() {
        let rows = vec![Ok(dummy_row()), Ok(dummy_row()), Ok(dummy_row())];
        let inner = stream::iter(rows).boxed();
        let mut ts = make_traced_stream(inner, Some("test"), Duration::from_secs(100));

        while ts.next().await.is_some() {}
        assert_eq!(ts.row_count, 3);
    }

    #[tokio::test]
    async fn empty_stream_yields_none_and_zero_rows() {
        let inner = stream::empty().boxed();
        let mut ts = make_traced_stream(inner, Some("test"), Duration::from_secs(100));
        assert!(ts.next().await.is_none());
        assert_eq!(ts.row_count, 0);
    }

    // ── Slow-path doesn't abort the stream ─────────────────────────

    #[tokio::test]
    async fn slow_query_still_yields_all_rows() {
        // Threshold is 0ms — every query is "slow", but all rows must still be returned.
        let rows = vec![Ok(dummy_row()), Ok(dummy_row())];
        let inner = stream::iter(rows).boxed();
        let mut ts = make_traced_stream(inner, Some("slow_q"), Duration::ZERO);

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
        let mut ts = make_traced_stream(inner, Some("slow_label"), Duration::ZERO);
        while ts.next().await.is_some() {}
        drop(ts);

        assert!(logs_contain("Slow Neo4j query"));
        assert!(logs_contain("slow_label"));
    }

    #[tokio::test]
    #[traced_test]
    async fn no_warning_when_under_threshold() {
        let inner = stream::empty().boxed();
        let ts = make_traced_stream(inner, Some("fast_q"), Duration::from_secs(600));
        drop(ts);

        assert!(!logs_contain("Slow Neo4j query"));
    }

    #[tokio::test]
    #[traced_test]
    async fn no_warning_when_label_is_none() {
        let inner = stream::empty().boxed();
        // threshold = 0 but no label → drop skips logging entirely
        let ts = make_traced_stream(inner, None, Duration::ZERO);
        drop(ts);

        assert!(!logs_contain("Slow Neo4j query"));
    }

    #[tokio::test]
    #[traced_test]
    async fn warning_includes_cypher_when_set() {
        let ts = TracedStream {
            inner: stream::empty().boxed(),
            label: Some("cypher_q"),
            cypher: Some("MATCH (n) RETURN n".into()),
            execute_duration: Duration::ZERO,
            stream_start: Instant::now(),
            row_count: 0,
            threshold: Duration::ZERO,
            metrics: GraphMetrics::new(),
        };
        drop(ts);

        assert!(logs_contain("MATCH (n) RETURN n"));
    }

    // ── TracedGraph ────────────────────────────────────────────────

    /// Mock `GraphOps` for testing `TracedGraph` without a real Neo4j connection.
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
    async fn traced_graph_execute_returns_all_rows() {
        let tg = TracedGraph::new(MockGraph { row_count: 3 });
        let mut stream = tg.execute(test_query()).await.unwrap();

        let mut count = 0;
        while stream.next().await.is_some() {
            count += 1;
        }
        assert_eq!(count, 3);
    }

    #[tokio::test]
    #[traced_test]
    async fn traced_graph_run_warns_on_slow_query() {
        let tg =
            TracedGraph::new(MockGraph { row_count: 0 }).with_slow_query_threshold(Duration::ZERO);
        tg.run(test_query()).await.unwrap();

        assert!(logs_contain("Slow Neo4j query"));
        assert!(logs_contain("test_label"));
    }

    #[tokio::test]
    #[traced_test]
    async fn traced_graph_run_no_warning_under_threshold() {
        let tg = TracedGraph::new(MockGraph { row_count: 0 })
            .with_slow_query_threshold(Duration::from_secs(600));
        tg.run(test_query()).await.unwrap();

        assert!(!logs_contain("Slow Neo4j query"));
    }
}
