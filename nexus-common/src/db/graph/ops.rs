use async_trait::async_trait;
use futures::stream::{self, BoxStream};
use futures::StreamExt;
use neo4rs::Row;

use super::query::Query;

/// Abstraction over graph database operations.
/// Callers depend on this trait, not the concrete implementations.
#[async_trait]
pub trait GraphOps: Send + Sync {
    /// Execute query, return boxed row stream.
    ///
    /// Neo4j starts the query before this returns, whether or not the stream is polled.
    async fn execute(
        &self,
        query: Query,
    ) -> neo4rs::Result<BoxStream<'static, Result<Row, neo4rs::Error>>>;

    /// Fire-and-forget query execution.
    async fn run(&self, query: Query) -> neo4rs::Result<()>;
}

/// Thin wrapper around `neo4rs::Graph` implementing `GraphOps` without any observability overhead.
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
        let rows = self.inner.execute(neo4rs::Query::from(query)).await?;
        // `into_stream` borrows the rows, so unfold over them to outlive this call
        let stream = stream::try_unfold(rows, |mut rows| async move {
            Ok(rows.next().await?.map(|row| (row, rows)))
        });
        Ok(stream.boxed())
    }

    async fn run(&self, query: Query) -> neo4rs::Result<()> {
        self.inner.run(neo4rs::Query::from(query)).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Neo4JConfig;
    use futures::TryStreamExt;
    use std::time::{Duration, Instant};
    use tokio::time::timeout;

    /// Keeps Neo4j busy for a while and returns no rows, so its only reply is the
    /// SUCCESS a dropped caller would leave unread on the connection.
    const SLOW_READ: &str = "UNWIND range(1, 50000000) AS x WITH x WHERE x < 0 RETURN x";
    /// The same work as a write that writes nothing: Neo4j skips a read whose
    /// results `run` discards.
    const SLOW_WRITE: &str = "UNWIND range(1, 50000000) AS x WITH x WHERE x < 0 CREATE ()";

    /// A pool of one connection, opened up front, so the probe's checkout meets the
    /// connection the dropped query held and the query's 50 ms aren't spent connecting.
    async fn single_connection_graph() -> neo4rs::Graph {
        let config = Neo4JConfig::default();
        let config = neo4rs::ConfigBuilder::default()
            .uri(&config.uri)
            .user(&config.user)
            .password(&config.password)
            .max_connections(1)
            .build()
            .unwrap();
        let graph = neo4rs::Graph::connect(config).unwrap();
        graph.run(neo4rs::query("RETURN 1")).await.unwrap();
        graph
    }

    async fn is_running(observer: &neo4rs::Graph, cypher: &str) -> bool {
        let query = neo4rs::query(
            "SHOW TRANSACTIONS YIELD currentQuery
             WHERE currentQuery = $cypher
             RETURN count(*) AS running",
        )
        .param("cypher", cypher);
        let mut rows = observer.execute(query).await.unwrap();
        let running: i64 = rows.next().await.unwrap().unwrap().get("running").unwrap();
        running > 0
    }

    /// Waits on another connection until Neo4j has run `cypher` to its last reply:
    /// first until it shows up, so a query that never ran fails, then until it is gone.
    async fn wait_until_finished(cypher: &str) {
        let observer = single_connection_graph().await;
        let deadline = Instant::now() + Duration::from_secs(30);
        for (running, state) in [(true, "started"), (false, "finished")] {
            while is_running(&observer, cypher).await != running {
                assert!(Instant::now() < deadline, "{cypher} never {state}");
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    }

    /// The next query on the pool must read its own reply.
    async fn assert_in_sync(graph: &Graph) {
        let query = Query::new("probe", "RETURN 42 AS answer");
        let mut rows = graph.execute(query).await.unwrap();
        let row = rows.try_next().await.unwrap().expect("the probe's own row");
        assert_eq!(row.get::<i64>("answer").unwrap(), 42);
    }

    #[tokio_shared_rt::test(shared)]
    async fn dropped_read_leaves_the_pool_in_sync() {
        let graph = Graph::new(single_connection_graph().await);

        let mut rows = graph
            .execute(Query::new("slow_read", SLOW_READ))
            .await
            .unwrap();
        let read = timeout(Duration::from_millis(50), rows.try_next()).await;
        assert!(
            read.is_err(),
            "the read finished before it could be dropped"
        );
        drop(rows);

        wait_until_finished(SLOW_READ).await;
        assert_in_sync(&graph).await;
    }

    #[tokio_shared_rt::test(shared)]
    async fn dropped_run_leaves_the_pool_in_sync() {
        let graph = Graph::new(single_connection_graph().await);

        let run = timeout(
            Duration::from_millis(50),
            graph.run(Query::new("slow_write", SLOW_WRITE)),
        )
        .await;
        assert!(
            run.is_err(),
            "the write finished before it could be dropped"
        );

        wait_until_finished(SLOW_WRITE).await;
        assert_in_sync(&graph).await;
    }
}
