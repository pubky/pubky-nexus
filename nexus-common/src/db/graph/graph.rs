use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::{StreamExt, TryStreamExt};
use neo4rs::{Graph, Row, Txn};
use std::time::{Duration, Instant};
use tracing::warn;

use super::query::{populate_cypher, Query};
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
        let cypher = query.cypher().to_owned();
        let params = query.params_map().clone();
        let start = Instant::now();
        let result = self.inner.execute(query.into()).await;
        let elapsed = start.elapsed();

        if elapsed > self.slow_query_threshold {
            warn!(elapsed_ms = elapsed.as_millis(), query = %populate_cypher(&cypher, &params), "Slow Neo4j query");
        }

        Ok(result?.into_stream().map_err(Into::into).boxed())
    }

    async fn run(&self, query: Query) -> neo4rs::Result<()> {
        let cypher = query.cypher().to_owned();
        let params = query.params_map().clone();
        let start = Instant::now();
        let result = self.inner.run(query.into()).await;
        let elapsed = start.elapsed();

        if elapsed > self.slow_query_threshold {
            warn!(elapsed_ms = elapsed.as_millis(), query = %populate_cypher(&cypher, &params), "Slow Neo4j query");
        }

        result
    }

    async fn start_txn(&self) -> neo4rs::Result<Txn> {
        self.inner.start_txn().await
    }
}
