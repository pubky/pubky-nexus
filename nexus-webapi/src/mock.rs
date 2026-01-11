use crate::{api_context::ApiContextBuilder, NexusApiBuilder};
use clap::ValueEnum;
use neo4rs::query;
use nexus_common::{
    db::{get_neo4j_graph, get_redis_conn, reindex},
    ApiConfig,
};
use std::process::Stdio;
use tracing::info;

#[derive(ValueEnum, Clone, Debug)]
pub enum MockType {
    Redis,
    Graph,
}

/// Provides utilities to mock and reset the Redis and Neo4j databases
/// Used for testing and ensuring a clean database state
pub struct MockDb {}

impl MockDb {
    pub async fn clear_database() {
        let api_context = ApiContextBuilder::from_default_config_dir()
            .api_config(ApiConfig::default())
            .try_build()
            .await
            .expect("Failed to create ApiContext");
        NexusApiBuilder(api_context)
            .init_stack()
            .await
            .expect("Failed to initialize stack");

        Self::drop_cache().await;
        Self::drop_graph().await;
        info!("Both ddbb cleared successfully");
    }

    pub async fn run(mock_type: Option<MockType>) {
        let api_context = ApiContextBuilder::from_default_config_dir()
            .api_config(ApiConfig::default())
            .try_build()
            .await
            .expect("Failed to create ApiContext");
        NexusApiBuilder(api_context)
            .init_stack()
            .await
            .expect("Failed to initialize stack");

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
            .run(drop_all_query)
            .await
            .expect("Could not drop graph nodes.");
    }

    pub async fn drop_cache() {
        info!("Dropping Redis database...");
        // Drop all keys in Redis
        let mut redis_conn = get_redis_conn()
            .await
            .expect("Could not get the redis connection");

        deadpool_redis::redis::cmd("FLUSHALL")
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
        // Run the run-queries.sh script on the Docker host using docker exec
        tokio::process::Command::new("docker")
            .args(["exec", "neo4j", "bash", "/test-graph/run-queries.sh"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await
            .expect("Failed to run run-queries.sh");
    }

    async fn sync_redis() {
        Self::drop_cache().await;
        // Reindex
        info!("Starting reindexing process...");
        reindex::sync().await;
    }
}
