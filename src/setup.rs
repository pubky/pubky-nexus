use crate::config::Config;
use crate::db::connectors::{
    neo4j::{Neo4jConnector, NEO4J_CONNECTOR},
    redis::{RedisConnector, REDIS_CONNECTOR},
};
use crate::db::graph::setup::setup_graph;
use log::{debug, info};

async fn setup_redis(config: &Config) {
    let redis_connector = RedisConnector::new_connection(&config.redis_uri())
        .await
        .expect("Failed to connect to Redis");

    match REDIS_CONNECTOR.set(redis_connector) {
        Err(e) => debug!("RedisConnector was already set: {:?}", e),
        Ok(()) => info!("RedisConnector successfully set"),
    }
}

async fn setup_neo4j(config: &Config) {
    let neo4j_connector = Neo4jConnector::new_connection(
        &config.neo4j_uri(),
        &config.neo4j_username,
        &config.neo4j_password,
    )
    .await
    .expect("Failed to connect to Neo4j");

    match NEO4J_CONNECTOR.set(neo4j_connector) {
        Err(e) => debug!("Neo4jConnector was already set: {:?}", e),
        Ok(()) => info!("Neo4jConnector successfully set"),
    }

    // Set Neo4J graph data constraints
    setup_graph().await.unwrap_or_default();
}

pub async fn setup(config: &Config) {
    match env_logger::try_init() {
        Ok(_) => info!("Env logger initiated"),
        Err(err) => debug!("Env logger was already set: {}", err),
    }

    // Initialize Redis and Neo4j
    setup_redis(config).await;
    setup_neo4j(config).await;
}
