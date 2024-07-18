use pk_social_common::connectors::neo4j::{Neo4jConnector, GLOBAL_NEO4J_CONNECTOR};
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
    let neo4j_connector = Neo4jConnector::new_connection(
        &config.neo4j_uri(),
        &config.neo4j_username,
        &config.neo4j_password,
    )
    .await
    .expect("Failed to connect to Neo4j");

    GLOBAL_NEO4J_CONNECTOR
        .set(neo4j_connector)
        .expect("Failed to set global Neo4j connector");

    // Routes
    let routes_v0 = routes::v0::routes();
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
