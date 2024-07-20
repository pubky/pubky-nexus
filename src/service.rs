use log::info;
use pubky_nexus::{routes, setup, Config};
use tokio::net::TcpListener;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[tokio::main]
async fn main() {
    let config = Config::from_env();
    setup(&config).await;

    // Routes
    let routes_v0 = routes::v0::routes();
    let route_static = routes::r#static::routes(&config.static_path);

    let app = routes_v0.merge(route_static).merge(
        SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", routes::v0::ApiDoc::openapi()),
    );

    // Start server
    let listener = TcpListener::bind(&config.server_binding()).await.unwrap();
    info!("LISTENING on {:?}\n", listener.local_addr().unwrap());
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
