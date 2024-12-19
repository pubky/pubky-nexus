use std::{process::Stdio, sync::Arc};

use anyhow::Result;
use log::info;
use neo4rs::query;
use pubky_nexus::{
    db::connectors::redis::get_redis_conn, get_neo4j_graph, reindex, routes, setup, Config,
};
use tokio::{
    net::TcpListener,
    sync::{Mutex, OnceCell},
};

// Util backend server for testing
// Performs the same routine the main service server does
// OnceCell is used to ensure the server is only started once
#[derive(Clone, Debug)]
pub struct TestServiceServer {
    pub initialized: bool,
}

pub static TEST_SERVER: OnceCell<Arc<Mutex<TestServiceServer>>> = OnceCell::const_new();

impl TestServiceServer {
    pub async fn get_test_server() -> Arc<Mutex<TestServiceServer>> {
        // Start the server if it hasn't been started
        TEST_SERVER
            .get_or_init(|| async {
                Self::start_server().await.unwrap();
                Arc::new(Mutex::new(TestServiceServer { initialized: true }))
            })
            .await
            .to_owned()
    }

    async fn sync_graph() {
        let graph = get_neo4j_graph().expect("Failed to get Neo4j graph connection");

        // drop and run the queries again
        let drop_all_query = query("MATCH (n) DETACH DELETE n;");
        graph
            .lock()
            .await
            .run(drop_all_query)
            .await
            .expect("Could not drop graph nodes.");

        // Run the run-queries.sh script on the Docker host using docker exec
        tokio::process::Command::new("docker")
            .args(&["exec", "neo4j", "bash", "/db-graph/run-queries.sh"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await
            .expect("Failed to run run-queries.sh");
    }

    async fn sync_redis() {
        // Drop all keys in Redis
        let mut redis_conn = get_redis_conn()
            .await
            .expect("Could not get the redis connection");

        redis::cmd("FLUSHALL")
            .exec_async(&mut redis_conn)
            .await
            .expect("Failed to flush Redis");

        // Reindex
        info!("Starting reindexing process.");
        reindex().await;
    }

    async fn start_server() -> Result<()> {
        let config = Config::from_env();
        setup(&config).await;

        // make sure DBs are in sync with mock data
        let sync_db_env = std::env::var("SYNC_DB").unwrap_or("false".to_string());
        match sync_db_env.as_str() {
            "true" => {
                Self::sync_graph().await;
                Self::sync_redis().await;
            }
            "graph" => {
                Self::sync_graph().await;
            }
            "false" => {}
            _ => {
                panic!("Invalid value for SYNC_DB");
            }
        }

        // App router
        let app = routes::routes();
        let listener = TcpListener::bind(&config.server_binding()).await.unwrap();
        info!("Listening on {:?}\n", listener.local_addr().unwrap());

        tokio::spawn(async {
            // Start server
            axum::serve(listener, app.into_make_service())
                .await
                .unwrap();
        });
        Ok(())
    }
}
