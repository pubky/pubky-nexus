use axum::Router;
use tower_http::cors::{Any, CorsLayer};

pub mod macros;
pub mod r#static;
pub mod v0;

mod middlewares;

pub fn routes() -> Router {
    let routes_v0 = v0::routes();
    let route_static = r#static::routes();

    // Combine routes
    let app = routes_v0.merge(route_static);

    // Create a CORS layer that allows all origins, methods, and headers
    let cors = CorsLayer::new()
        .allow_origin(Any) // Allow all origins
        .allow_methods(Any) // Allow all HTTP methods
        .allow_headers(Any); // Allow all headers

    // Layer the CORS middleware on top of the routes
    app.layer(axum::middleware::from_fn(
        middlewares::tracing::tracing_middleware,
    ))
    .layer(cors)
}
