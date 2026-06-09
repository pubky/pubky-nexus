use clap::ValueEnum;
use nexus_common::{
    db::{get_neo4j_graph, get_redis_conn, graph::Query, reindex},
    models::post::create_post_content_index,
    StackConfig, StackManager,
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
    async fn init_stack() {
        StackManager::setup(&StackConfig::default())
            .await
            .expect("Failed to initialize stack");
    }

    pub async fn clear_database() {
        Self::init_stack().await;

        Self::drop_cache().await;
        Self::drop_graph().await;
        info!("Both ddbb cleared successfully");
    }

    pub async fn run(mock_type: Option<MockType>) {
        Self::init_stack().await;

        match mock_type {
            Some(MockType::Redis) => Self::sync_redis().await,
            Some(MockType::Graph) => Self::sync_graph().await,
            None => Self::sync_all().await,
        }
    }

    async fn drop_graph() {
        info!("Dropping Graph database...");
        let graph = get_neo4j_graph().expect("Failed to get Neo4j graph connection");

        // Batch deletion avoids blowing Neo4j's transaction memory limit on large graphs.
        let drop_all_query = Query::new(
            "drop_graph",
            "CALL { MATCH (n) WITH n LIMIT 10000 DETACH DELETE n } IN TRANSACTIONS OF 10000 ROWS;",
        );
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

        // Allow other runtimes like podman, but default to docker
        let container_runtime = std::env::var("CONTAINER_RUNTIME").unwrap_or("docker".to_string());

        // Run the run-queries.sh script inside the neo4j container using docker exec
        tokio::process::Command::new(&container_runtime)
            .args(["exec", "neo4j", "bash", "/test-graph/run-queries.sh"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await
            .expect("Failed to run run-queries.sh");
    }

    async fn sync_redis() {
        Self::drop_cache().await;
        // TODO: test framework should run migrations after resetting to a fresh database;
        // that would recreate this index automatically and remove the need for this call.
        create_post_content_index()
            .await
            .expect("Failed to create post content FT index");
        info!("Starting reindexing process...");
        reindex::sync().await;
    }
}
