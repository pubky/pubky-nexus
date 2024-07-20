use crate::config::Config;
use env_logger;
use log::{error, info};
use pk_social_common::connectors::{
    neo4j::{Neo4jConnector, NEO4J_CONNECTOR},
    redis::{RedisConnector, REDIS_CONNECTOR},
};

pub async fn setup(config: &Config) {
    env_logger::init();

    // Initialize Neo4j connection
    let neo4j_connector = Neo4jConnector::new_connection(
        &config.neo4j_uri(),
        &config.neo4j_username,
        &config.neo4j_password,
    )
    .await
    .expect("Failed to connect to Neo4j");

    if NEO4J_CONNECTOR.set(neo4j_connector).is_err() {
        error!("Neo4jConnector already set");
    } else {
        info!("RedisConnector successfully set");
    }

    // Initialize Redis connection
    let redis_connector = RedisConnector::new_connection(&config._redis_uri())
        .await
        .expect("Failed to connect to Redis");

    if REDIS_CONNECTOR.set(redis_connector).is_err() {
        error!("RedisConnector already set");
    } else {
        info!("RedisConnector successfully set");
    }
}
