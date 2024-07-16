use tokio::net::TcpListener;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod routes;

#[tokio::main]
async fn main() {
    let routes_v0 = routes::v0::create_routes();

    let app = routes_v0.merge(SwaggerUi::new("/swagger-ui").url(
        "/api-docs/openapi.json",
        routes::v0::info::ApiDoc::openapi(),
    ));

    // start server
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    println!("->> LISTENING on {:?}\n", listener.local_addr());
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
