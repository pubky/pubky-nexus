use crate::config::Config;
use crate::db::connectors::pubky::{PubkyConnector, PUBKY_CONNECTOR};
use crate::db::connectors::{
    neo4j::{Neo4jConnector, NEO4J_CONNECTOR},
    redis::{RedisConnector, REDIS_CONNECTOR},
};
use crate::db::graph::setup::setup_graph;
use log::{error, info};
use pkarr::mainline::Testnet;
use pubky::PubkyClient;

async fn setup_redis(config: &Config) {
    let redis_connector = RedisConnector::new_connection(&config.redis_uri())
        .await
        .expect("Failed to connect to Redis");

    match REDIS_CONNECTOR.set(redis_connector) {
        Err(e) => error!("RedisConnector was already set: {:?}", e),
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
        Err(e) => error!("Neo4jConnector was already set: {:?}", e),
        Ok(()) => info!("Neo4jConnector successfully set"),
    }

    // Set Neo4J graph data constraints
    setup_graph().await.unwrap_or_default();
}

pub async fn setup_pubky(config: &Config) {
    let pubky_client = if config.testnet {
        let testnet = Testnet {
            bootstrap: vec![config.bootstrap.clone()],
            nodes: vec![],
        };
        PubkyClient::test(&testnet)
    } else {
        PubkyClient::default()
    };

    let pubky_connector = PubkyConnector::new_connection(pubky_client)
        .await
        .expect("Failed to connect to Pubky");

    match PUBKY_CONNECTOR.set(pubky_connector) {
        Err(e) => error!("PubkyConnector was already set: {:?}", e),
        Ok(()) => info!("PubkyConnector successfully set"),
    }
}

pub async fn setup(config: &Config) {
    match env_logger::try_init() {
        Ok(_) => info!("Env logger initiated"),
        Err(err) => error!("Env logger was already set: {}", err),
    }

    // Initialize Redis, Neo4j and the Pubky client
    setup_redis(config).await;
    setup_neo4j(config).await;
    setup_pubky(config).await;
}
