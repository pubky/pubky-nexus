use clap::ValueEnum;
use tracing::info;

use std::process::Stdio;

use neo4rs::query;

use crate::{_service::NexusApi, get_neo4j_graph, get_redis_conn, reindex};

#[derive(ValueEnum, Clone, Debug)]
pub enum MockType {
    Redis,
    Graph,
}

pub struct MockDb {}

impl MockDb {
    pub async fn clear_database() {
        NexusApi::builder().init_stack().await;
        Self::drop_cache().await;
        Self::drop_graph().await;
        info!("Both ddbb cleared successfully");
    }

    pub async fn run(mock_type: Option<MockType>) {
        NexusApi::builder().init_stack().await;
        match mock_type {
            Some(MockType::Redis) => Self::sync_redis().await,
            Some(MockType::Graph) => Self::sync_graph().await,
            None => Self::sync_all().await,
        }
    }

    async fn drop_graph() {
        info!("Dropping Graph database...");
        let graph = get_neo4j_graph().expect("Failed to get Neo4j graph connection");

        // drop and run the queries again
        let drop_all_query = query("MATCH (n) DETACH DELETE n;");
        graph
            .lock()
            .await
            .run(drop_all_query)
            .await
            .expect("Could not drop graph nodes.");
    }

    async fn drop_cache() {
        info!("Dropping Redis database...");
        // Drop all keys in Redis
        let mut redis_conn = get_redis_conn()
            .await
            .expect("Could not get the redis connection");

        redis::cmd("FLUSHALL")
            .exec_async(&mut redis_conn)
            .await
            .expect("Failed to flush Redis");
    }

    async fn sync_all() {
        info!("Mocking both Redis and Graph databases...");
        Self::sync_graph().await;
        Self::sync_redis().await;
    }

    async fn sync_graph() {
        Self::drop_graph().await;
        //let graph_env = std::env::var("GRAPH_CONTAINER_NAME").unwrap_or("neo4j".to_string());
        // Run the run-queries.sh script on the Docker host using docker exec
        tokio::process::Command::new("docker")
            .args(["exec", "neo4j", "bash", "/db-graph/run-queries.sh"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await
            .expect("Failed to run run-queries.sh");
    }

    async fn sync_redis() {
        Self::drop_cache().await;
        // Reindex
        info!("Starting reindexing process.");
        reindex().await;
    }
}
