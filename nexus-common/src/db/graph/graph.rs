use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::{StreamExt, TryStreamExt};
use neo4rs::{Graph, Row, Txn};
use std::time::{Duration, Instant};
use tracing::{debug, error, warn};

use super::query::Query;
use crate::db::config::DEFAULT_SLOW_QUERY_THRESHOLD_MS;

/// Abstraction over graph database operations.
/// Callers depend on this trait, not the concrete TracedGraph.
#[async_trait]
pub trait GraphExec: Send + Sync {
    /// Execute query, return boxed row stream.
    async fn execute(
        &self,
        query: Query,
    ) -> neo4rs::Result<BoxStream<'static, Result<Row, neo4rs::Error>>>;

    /// Fire-and-forget query execution.
    async fn run(&self, query: Query) -> neo4rs::Result<()>;

    /// Start a transaction.
    async fn start_txn(&self) -> neo4rs::Result<Txn>;
}

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
        let start = Instant::now();
        let populated_cypher = query.to_cypher_populated();
        let result = self.inner.execute(query.into()).await;
        let elapsed = start.elapsed();

        match &result {
            Ok(_) if elapsed > self.slow_query_threshold => {
                warn!(elapsed_ms = elapsed.as_millis(), query = %populated_cypher, "Slow Neo4j query");
            }
            Ok(_) => {
                debug!(elapsed_ms = elapsed.as_millis(), query = %populated_cypher, "Neo4j query");
            }
            Err(e) => {
                error!(elapsed_ms = elapsed.as_millis(), query = %populated_cypher, error = %e, "Neo4j query failed");
            }
        }

        Ok(result?.into_stream().map_err(Into::into).boxed())
    }

    async fn run(&self, query: Query) -> neo4rs::Result<()> {
        let start = Instant::now();
        let populated_cypher = query.to_cypher_populated();
        let result = self.inner.run(query.into()).await;
        let elapsed = start.elapsed();

        match &result {
            Ok(()) if elapsed > self.slow_query_threshold => {
                warn!(elapsed_ms = elapsed.as_millis(), query = %populated_cypher, "Slow Neo4j run");
            }
            Ok(()) => {
                debug!(elapsed_ms = elapsed.as_millis(), query = %populated_cypher, "Neo4j run");
            }
            Err(e) => {
                error!(elapsed_ms = elapsed.as_millis(), query = %populated_cypher, error = %e, "Neo4j run failed");
            }
        }

        result
    }

    async fn start_txn(&self) -> neo4rs::Result<Txn> {
        self.inner.start_txn().await
    }
}
