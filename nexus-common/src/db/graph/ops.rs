use async_trait::async_trait;
use futures::stream::{self, BoxStream};
use futures::StreamExt;
use neo4rs::Row;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, oneshot};
use tracing::warn;

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

/// Tags every query so [`Graph::stop`] can find its transaction. Neo4j ignores
/// parameters a query doesn't use.
const QUERY_ID: &str = "nexus_query_id";

/// Thin wrapper around `neo4rs::Graph` implementing `GraphOps` without any observability overhead.
///
/// Each query runs in its own task, so a caller that is dropped mid-query (a cancelled
/// request, a `timeout`) can't stop it halfway through reading a reply. neo4rs 0.8 would
/// return that connection to the pool one reply behind, and the next query on it would
/// read the wrong reply (#1149). The task terminates the query instead, then reads the
/// reply that ends it, so the connection goes back in sync and right away.
#[derive(Clone)]
pub struct Graph {
    inner: neo4rs::Graph,
    /// Its own connection, so stopping a query never waits behind the queries it stops.
    stopper: neo4rs::Graph,
}

impl Graph {
    pub fn new(graph: neo4rs::Graph, stopper: neo4rs::Graph) -> Self {
        Self {
            inner: graph,
            stopper,
        }
    }

    /// Drives one step of a query (RUN, or reading a batch) to its end: dropping it
    /// mid-reply is the bug. If the caller goes away first, terminates the query so the
    /// step ends now rather than when Neo4j finishes it.
    async fn finish<T>(
        &self,
        id: &str,
        step: impl Future<Output = T>,
        caller_gone: impl Future<Output = ()>,
    ) -> T {
        tokio::pin!(step);
        tokio::select! {
            out = &mut step => return out,
            () = caller_gone => {}
        }
        let mut warned = false;
        loop {
            match self.stop(id).await {
                Err(e) if !warned => {
                    warned = true;
                    warn!(error = %e, "Could not terminate an abandoned Neo4j query");
                }
                _ => {}
            }
            // A query Neo4j hasn't registered yet isn't listed, so try again.
            if let Ok(out) = tokio::time::timeout(Duration::from_secs(1), &mut step).await {
                return out;
            }
        }
    }

    async fn stop(&self, id: &str) -> neo4rs::Result<()> {
        let query = neo4rs::query(&format!(
            "SHOW TRANSACTIONS YIELD transactionId AS tx, parameters
             WHERE parameters.{QUERY_ID} = $id
             TERMINATE TRANSACTIONS tx YIELD message RETURN message"
        ))
        .param("id", id);
        // Read the rows: Neo4j skips the termination if its result is discarded.
        let mut rows = self.stopper.execute(query).await?;
        while rows.next().await?.is_some() {}
        Ok(())
    }
}

#[async_trait]
impl GraphOps for Graph {
    async fn execute(
        &self,
        query: Query,
    ) -> neo4rs::Result<BoxStream<'static, Result<Row, neo4rs::Error>>> {
        let graph = self.clone();
        let (mut started_tx, started_rx) = oneshot::channel();
        let (rows_tx, rows_rx) = mpsc::channel(1);
        tokio::spawn(async move {
            let (id, query) = tagged(query);
            let started = graph.inner.execute(query);
            let mut rows = match graph.finish(&id, started, started_tx.closed()).await {
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
            while !rows_tx.is_closed() {
                let next = graph.finish(&id, rows.next(), rows_tx.closed()).await;
                let Some(row) = next.transpose() else { break };
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
        let graph = self.clone();
        let (mut done_tx, done_rx) = oneshot::channel();
        tokio::spawn(async move {
            let (id, query) = tagged(query);
            let run = graph.inner.run(query);
            let result = graph.finish(&id, run, done_tx.closed()).await;
            let _ = done_tx.send(result);
        });
        done_rx.await.map_err(task_failed)?
    }
}

/// Converts `query`, tagged with an id unique across processes sharing the database.
fn tagged(query: Query) -> (String, neo4rs::Query) {
    static PROCESS: LazyLock<String> = LazyLock::new(|| {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        format!("{}-{nanos}", std::process::id())
    });
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let id = format!("{}-{}", *PROCESS, NEXT.fetch_add(1, Ordering::Relaxed));
    let query = neo4rs::Query::from(query).param(QUERY_ID, id.as_str());
    (id, query)
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

    /// Waits on the observer's connection until Neo4j has run `cypher` to its last reply.
    async fn wait_until_finished(observer: &neo4rs::Graph, cypher: &str) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while is_running(observer, cypher).await {
            assert!(Instant::now() < deadline, "{cypher} never finished");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// The next query on the pool must read its own reply.
    async fn assert_in_sync(graph: &Graph) {
        let query = Query::new("probe", "RETURN 42 AS answer");
        let mut rows = graph.execute(query).await.unwrap();
        let row = rows.try_next().await.unwrap().expect("the probe's own row");
        assert_eq!(row.get::<i64>("answer").unwrap(), 42);
    }

    /// Drops `caller` once it has waited `after`, checking first, while the caller still
    /// holds the query, that Neo4j is running it: once dropped, it may end at once.
    async fn drop_while_running<F: Future>(
        caller: F,
        after: Duration,
        observer: &neo4rs::Graph,
        cypher: &str,
    ) {
        let mut caller = Box::pin(caller);
        assert!(
            timeout(after, &mut caller).await.is_err(),
            "{cypher} finished before it could be dropped"
        );
        assert!(is_running(observer, cypher).await, "{cypher} isn't running");
    }

    async fn graph_and_observer() -> (Graph, neo4rs::Graph) {
        let graph = Graph::new(
            single_connection_graph().await,
            single_connection_graph().await,
        );
        (graph, single_connection_graph().await)
    }

    #[tokio_shared_rt::test(shared)]
    async fn dropped_read_leaves_the_pool_in_sync() {
        let (graph, observer) = graph_and_observer().await;

        let query = Query::new("slow_read", SLOW_READ);
        let mut rows = graph.execute(query).await.unwrap();
        let read = async move { rows.try_next().await };
        drop_while_running(read, Duration::from_millis(50), &observer, SLOW_READ).await;

        wait_until_finished(&observer, SLOW_READ).await;
        assert_in_sync(&graph).await;
    }

    #[tokio_shared_rt::test(shared)]
    async fn dropped_run_leaves_the_pool_in_sync() {
        let (graph, observer) = graph_and_observer().await;

        let run = graph.run(Query::new("slow_write", SLOW_WRITE));
        drop_while_running(run, Duration::from_millis(50), &observer, SLOW_WRITE).await;

        wait_until_finished(&observer, SLOW_WRITE).await;
        assert_in_sync(&graph).await;
    }

    /// About 12 s of work each, far longer than the probe may wait.
    const VERY_SLOW_READ: &str = "UNWIND range(1, 500000000) AS x WITH x WHERE x < 0 RETURN x";
    const VERY_SLOW_WRITE: &str = "UNWIND range(1, 500000000) AS x WITH x WHERE x < 0 CREATE ()";

    /// A dropped query must not hold its connection until Neo4j finishes it: the next
    /// query on the one-connection pool answers at once, and the dropped one is gone.
    async fn assert_released_at_once(graph: &Graph, observer: &neo4rs::Graph, cypher: &str) {
        let start = Instant::now();
        assert_in_sync(graph).await;
        let waited = start.elapsed();
        assert!(
            waited < Duration::from_secs(3),
            "the probe waited {waited:?}"
        );
        assert!(!is_running(observer, cypher).await, "{cypher} still runs");
    }

    #[tokio_shared_rt::test(shared)]
    async fn dropped_read_releases_its_connection_at_once() {
        let (graph, observer) = graph_and_observer().await;

        let query = Query::new("very_slow_read", VERY_SLOW_READ);
        let mut rows = graph.execute(query).await.unwrap();
        let read = async move { rows.try_next().await };
        drop_while_running(read, Duration::from_millis(300), &observer, VERY_SLOW_READ).await;

        assert_released_at_once(&graph, &observer, VERY_SLOW_READ).await;
    }

    #[tokio_shared_rt::test(shared)]
    async fn dropped_run_releases_its_connection_at_once() {
        let (graph, observer) = graph_and_observer().await;

        let run = graph.run(Query::new("very_slow_write", VERY_SLOW_WRITE));
        drop_while_running(run, Duration::from_millis(300), &observer, VERY_SLOW_WRITE).await;

        assert_released_at_once(&graph, &observer, VERY_SLOW_WRITE).await;
    }
}
