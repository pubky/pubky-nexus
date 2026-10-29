use async_trait::async_trait;
use futures::stream::{self, BoxStream};
use futures::StreamExt;
use neo4rs::Row;
use tokio::sync::{mpsc, oneshot};

use super::query::Query;

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

/// Thin wrapper around `neo4rs::Graph` implementing `GraphOps` without any observability overhead.
///
/// Each query runs in its own task, so a caller that is dropped mid-query (a cancelled
/// request, a `timeout`) can't stop it halfway through reading a reply. neo4rs 0.8 would
/// return that connection to the pool one reply behind, and the next query on it would
/// read the wrong reply (#1149).
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
        let graph = self.inner.clone();
        let (started_tx, started_rx) = oneshot::channel();
        let (rows_tx, rows_rx) = mpsc::channel(1);
        tokio::spawn(async move {
            let mut rows = match graph.execute(query.into()).await {
                Ok(rows) => rows,
                Err(e) => {
                    let _ = started_tx.send(Err(e));
                    return;
                }
            };
            if started_tx.send(Ok(())).is_err() {
                return; // the caller is gone
            }
            // `next` returns only once a whole batch is read, so stopping between two
            // calls leaves the connection in sync; the pool's RESET discards the rest.
            while let Some(row) = rows.next().await.transpose() {
                let failed = row.is_err();
                if rows_tx.send(row).await.is_err() || failed {
                    break;
                }
            }
        });

        started_rx.await.map_err(task_failed)??;
        let rows = stream::unfold(rows_rx, |mut rx| async {
            rx.recv().await.map(|row| (row, rx))
        });
        Ok(rows.boxed())
    }

    async fn run(&self, query: Query) -> neo4rs::Result<()> {
        let graph = self.inner.clone();
        tokio::spawn(async move { graph.run(query.into()).await })
            .await
            .map_err(task_failed)?
    }
}

/// The query task panicked, or the runtime is shutting down.
fn task_failed(e: impl std::error::Error + Send + Sync + 'static) -> neo4rs::Error {
    neo4rs::Error::IOError {
        detail: std::io::Error::other(e),
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
        let graph = neo4rs::Graph::connect(config).await.unwrap();
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
