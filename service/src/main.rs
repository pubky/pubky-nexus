use pk_social_common::connectors::neo4j::Neo4jConnector;
use std::sync::Arc;
use tokio::net::TcpListener;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod config;
mod error;
mod models;
mod routes;

pub use self::error::{Error, Result};

#[tokio::main]
async fn main() {
    let config = config::Config::from_env();

    // Initialize Neo4j connection
    let neo4j_connector = Arc::new(Neo4jConnector::new());
    neo4j_connector
        .connect(
            &config.neo4j_uri(),
            &config.neo4j_username,
            &config.neo4j_password,
        )
        .await
        .expect("Failed to connect to Neo4j");

    // Routes
    let routes_v0 = routes::v0::routes(neo4j_connector.clone());
    let route_static = routes::r#static::routes(&config.static_path);

    let app = routes_v0.merge(route_static).merge(
        SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", routes::v0::ApiDoc::openapi()),
    );

    // Start server
    let listener = TcpListener::bind(&config.server_binding()).await.unwrap();
    println!("->> LISTENING on {:?}\n", listener.local_addr());
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
