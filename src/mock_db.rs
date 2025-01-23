use log::info;

use std::process::Stdio;

use neo4rs::query;

use pubky_nexus::{
    db::connectors::redis::get_redis_conn, get_neo4j_graph, reindex, Config, StackManager,
};

#[tokio::main]
async fn main() {
    let config = Config::from_env();
    StackManager::setup(&config).await;
    info!("Running mock db sync");
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("graph") => {
            MockDB::sync_graph().await;
        }
        Some("redis") => {
            MockDB::sync_redis().await;
        }
        None => {
            MockDB::sync_graph().await;
            MockDB::sync_redis().await;
        }
        Some(_) => {
            panic!("Invalid argument. Use 'graph' or 'redis'");
        }
    }
}

pub struct MockDB {}

impl MockDB {
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

        let graph_env = std::env::var("GRAPH_CONTAINER_NAME").unwrap_or("neo4j".to_string());
        // Run the run-queries.sh script on the Docker host using docker exec
        tokio::process::Command::new("docker")
            .args(&[
                "exec",
                graph_env.as_str(),
                "bash",
                "/db-graph/run-queries.sh",
            ])
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
}
