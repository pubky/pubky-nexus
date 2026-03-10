use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::{Stream, StreamExt, TryStreamExt};
use neo4rs::Row;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use tracing::warn;

use super::query::Query;
use crate::db::config::DEFAULT_SLOW_QUERY_THRESHOLD_MS;

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

/// A stream wrapper that measures total query time and logs slow queries when dropped.
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
}

impl Stream for TracedStream {
    type Item = Result<Row, neo4rs::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let result = Pin::new(&mut self.inner).poll_next(cx);
        if let Poll::Ready(Some(Ok(_))) = &result {
            self.row_count += 1;
        }
        result
    }
}

impl Drop for TracedStream {
    fn drop(&mut self) {
        if let Some(label) = &self.label {
            let fetch_duration = self.stream_start.elapsed();
            let total = self.execute_duration + fetch_duration;
            if total > self.threshold {
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

/// Decorator around a [`GraphOps`] implementation that logs slow queries.
#[derive(Clone)]
pub struct TracedGraph<G = Graph> {
    inner: G,
    slow_query_threshold: Duration,
    log_cypher: bool,
}

impl<G: GraphOps> TracedGraph<G> {
    pub fn new(graph: G) -> Self {
        Self {
            inner: graph,
            slow_query_threshold: Duration::from_millis(DEFAULT_SLOW_QUERY_THRESHOLD_MS),
            log_cypher: false,
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
        let stream = self.inner.execute(query).await;
        let execute_duration = start.elapsed();

        // Log slow queries that fail — TracedStream::drop won't fire for errored executes.
        if stream.is_err() && execute_duration > self.slow_query_threshold {
            if let Some(lbl) = &label {
                warn!(
                    execute_ms = execute_duration.as_millis(),
                    query = %lbl,
                    cypher = cypher.as_deref().unwrap_or(""),
                    "Slow Neo4j query (execute failed)"
                );
            }
        }

        let traced = TracedStream {
            inner: stream?,
            label,
            cypher,
            execute_duration,
            stream_start: Instant::now(), // Timestamp after execute(), so it only tracks the fetch phase
            row_count: 0,
            threshold: self.slow_query_threshold,
        };
        Ok(traced.boxed())
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

        if let Some(label) = &label {
            if elapsed > self.slow_query_threshold {
                warn!(elapsed_ms = elapsed.as_millis(), query = %label, cypher = cypher.as_deref().unwrap_or(""), "Slow Neo4j query");
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
