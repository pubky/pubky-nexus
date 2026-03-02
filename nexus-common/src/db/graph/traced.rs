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
pub trait GraphExec: Send + Sync {
    /// Execute query, return boxed row stream.
    async fn execute(
        &self,
        query: Query,
    ) -> neo4rs::Result<BoxStream<'static, Result<Row, neo4rs::Error>>>;

    /// Fire-and-forget query execution.
    async fn run(&self, query: Query) -> neo4rs::Result<()>;
}

/// Thin wrapper around `neo4rs::Graph` implementing `GraphExec` without tracing.
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
impl GraphExec for Graph {
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
    /// Pool-acquire + Bolt RUN round-trip (query planning & start of execution).
    execute_duration: Duration,
    /// Cumulative time spent inside poll_next (row fetching).
    fetch_duration: Duration,
    row_count: usize,
    threshold: Duration,
}

impl Stream for TracedStream {
    type Item = Result<Row, neo4rs::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let poll_start = Instant::now();
        let result = Pin::new(&mut self.inner).poll_next(cx);
        self.fetch_duration += poll_start.elapsed();
        if let Poll::Ready(Some(Ok(_))) = &result {
            self.row_count += 1;
        }
        result
    }
}

impl Drop for TracedStream {
    fn drop(&mut self) {
        if let Some(label) = &self.label {
            let total = self.execute_duration + self.fetch_duration;
            if total > self.threshold {
                warn!(
                    total_ms = total.as_millis(),
                    execute_ms = self.execute_duration.as_millis(),
                    fetch_ms = self.fetch_duration.as_millis(),
                    rows = self.row_count,
                    query = %label,
                    "Slow Neo4j query"
                );
            }
        }
    }
}

/// Decorator around `Graph` that logs slow queries.
#[derive(Clone)]
pub struct TracedGraph {
    inner: Graph,
    slow_query_threshold: Duration,
}

impl TracedGraph {
    pub fn new(graph: Graph) -> Self {
        Self {
            inner: graph,
            slow_query_threshold: Duration::from_millis(DEFAULT_SLOW_QUERY_THRESHOLD_MS),
        }
    }

    pub fn with_slow_query_threshold(mut self, threshold: Duration) -> Self {
        self.slow_query_threshold = threshold;
        self
    }
}

#[async_trait]
impl GraphExec for TracedGraph {
    async fn execute(
        &self,
        query: Query,
    ) -> neo4rs::Result<BoxStream<'static, Result<Row, neo4rs::Error>>> {
        let label = query.label();
        let start = Instant::now();
        let stream = self.inner.execute(query).await?;
        let execute_duration = start.elapsed();

        let traced = TracedStream {
            inner: stream,
            label,
            execute_duration,
            fetch_duration: Duration::ZERO,
            row_count: 0,
            threshold: self.slow_query_threshold,
        };
        Ok(traced.boxed())
    }

    async fn run(&self, query: Query) -> neo4rs::Result<()> {
        let label = query.label();
        let start = Instant::now();
        let result = self.inner.run(query).await;
        let elapsed = start.elapsed();

        if let Some(label) = &label {
            if elapsed > self.slow_query_threshold {
                warn!(elapsed_ms = elapsed.as_millis(), query = %label, "Slow Neo4j query");
            }
        }

        result
    }
}
