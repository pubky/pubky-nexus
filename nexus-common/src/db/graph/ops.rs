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
        // Not `neo4rs::Graph::run`: it retries any error on the reply that ends the
        // query, for up to 60s, so a constraint violation or a reply timeout would
        // re-run the query. `execute` retries transient errors only, as 0.8 did.
        let mut rows = self.inner.execute(neo4rs::Query::from(query)).await?;
        while rows.next().await?.is_some() {}
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Neo4JConfig;
    use futures::TryStreamExt;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
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

    /// A Bolt 4.4 server that accepts every RUN and fails the reply that ends it
    /// with a client error, the way a commit-time failure arrives. Counts RUNs.
    async fn failing_after_run_server() -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let uri = format!("bolt://{}", listener.local_addr().unwrap());
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = runs.clone();
        tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let runs = counter.clone();
                tokio::spawn(async move {
                    let mut handshake = [0; 20];
                    socket.read_exact(&mut handshake).await?;
                    socket.write_all(&[0, 0, 4, 4]).await?;
                    const SUCCESS: &[u8] = &[0xB1, 0x70, 0xA0];
                    const RUN_SUCCESS: &[u8] = &[
                        0xB1, 0x70, 0xA1, 0x86, b'f', b'i', b'e', b'l', b'd', b's', 0x90,
                    ];
                    let code = b"Neo.ClientError.Schema.ConstraintValidationFailed";
                    let mut failure = vec![0xB1, 0x7F, 0xA2, 0x84, b'c', b'o', b'd', b'e', 0xD0];
                    failure.push(code.len() as u8);
                    failure.extend_from_slice(code);
                    failure.extend_from_slice(&[0x87, b'm', b'e', b's', b's', b'a', b'g', b'e']);
                    failure.extend_from_slice(&[0x84, b'f', b'a', b'i', b'l']);
                    let mut failed = false;
                    loop {
                        // One message: chunks up to the 0x0000 end marker.
                        let mut message = Vec::new();
                        loop {
                            let mut len = [0; 2];
                            socket.read_exact(&mut len).await?;
                            let len = u16::from_be_bytes(len) as usize;
                            if len == 0 {
                                break;
                            }
                            let mut chunk = vec![0; len];
                            socket.read_exact(&mut chunk).await?;
                            message.extend(chunk);
                        }
                        let reply: &[u8] = match message.get(1) {
                            Some(0x10) if failed => &[0xB0, 0x7E], // IGNORED until RESET
                            Some(0x10) => {
                                runs.fetch_add(1, Ordering::SeqCst);
                                RUN_SUCCESS
                            }
                            Some(0x2F | 0x3F) if failed => &[0xB0, 0x7E],
                            Some(0x2F | 0x3F) => {
                                failed = true;
                                &failure
                            }
                            Some(0x0F) => {
                                failed = false;
                                SUCCESS
                            }
                            Some(0x02) | None => return Ok::<(), std::io::Error>(()),
                            _ => SUCCESS,
                        };
                        let mut frame = (reply.len() as u16).to_be_bytes().to_vec();
                        frame.extend_from_slice(reply);
                        frame.extend_from_slice(&[0, 0]);
                        socket.write_all(&frame).await?;
                    }
                });
            }
        });
        (uri, runs)
    }

    /// neo4rs's own `run` retries any error on the reply that ends the query, for
    /// up to 60s, re-running the query each time. Ours must fail on the first.
    #[tokio::test]
    async fn run_does_not_retry_a_client_error() {
        let (uri, runs) = failing_after_run_server().await;
        let config = neo4rs::ConfigBuilder::default()
            .uri(&uri)
            .user("neo4j")
            .password("neo4j")
            .build()
            .unwrap();
        let graph = Graph::new(neo4rs::Graph::connect(config).unwrap());

        let result = timeout(
            Duration::from_secs(5),
            graph.run(Query::new("write", "CREATE (:Post {id: 'taken'})")),
        )
        .await
        .expect("a client error should not be retried");

        assert!(result.is_err(), "got {result:?}");
        assert_eq!(
            runs.load(Ordering::SeqCst),
            1,
            "the query ran more than once"
        );
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
