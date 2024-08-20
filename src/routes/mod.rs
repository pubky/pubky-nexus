use axum::{Extension, Router};
use tower_http::cors::{Any, CorsLayer};

pub mod macros;
pub mod r#static;
pub mod v0;

pub fn routes() -> Router {
    let routes_v0 = v0::routes();
    let route_static = r#static::routes();

    let cors_middleware = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(vec!["GET".parse().unwrap(), "POST".parse().unwrap()])
        .allow_headers(vec!["Content-Type".parse().unwrap()]);

    Router::new()
        .merge(routes_v0)
        .merge(route_static)
        .layer(cors_middleware)
        .layer(Extension(()))
}
