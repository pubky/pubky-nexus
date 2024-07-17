use dotenv::dotenv;
use pk_social_common::connectors::neo4j::Neo4jConnector;
use std::env;
use std::sync::Arc;
use tokio::net::TcpListener;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod error;
mod models;
mod routes;

pub use self::error::{Error, Result};

#[tokio::main]
async fn main() {
    dotenv().ok();

    // Initialize Neo4j connection
    let neo4j_host = env::var("NEO4J_HOST").expect("NEO4J_HOST environment variable not set");
    let neo4j_port = env::var("NEO4J_PORT").expect("NEO4J_PORT environment variable not set");
    let neo4j_username =
        env::var("NEO4J_DB_USERNAME").expect("NEO4J_DB_USERNAME environment variable not set");
    let neo4j_password =
        env::var("NEO4J_PASSWORD").expect("NEO4J_PASSWORD environment variable not set");

    let neo4j_uri = format!("bolt://{}:{}", neo4j_host, neo4j_port);
    let neo4j_connector = Arc::new(Neo4jConnector::new());
    neo4j_connector
        .connect(&neo4j_uri, &neo4j_username, &neo4j_password)
        .await
        .expect("Failed to connect to Neo4j");

    // Routes
    let routes_v0 = routes::v0::routes(neo4j_connector.clone());
    let route_static = routes::r#static::routes();

    let app = routes_v0.merge(route_static).merge(
        SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", routes::v0::ApiDoc::openapi()),
    );

    // Start server
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    println!("->> LISTENING on {:?}\n", listener.local_addr());
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
