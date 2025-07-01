use axum::Router;
use std::{path::PathBuf, sync::Arc};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use utoipa_swagger_ui::SwaggerUi;

pub mod macros;
pub mod r#static;
pub mod v0;

mod middlewares;

#[derive(Clone)]
pub struct AppState {
    pub files_path: Arc<PathBuf>,
}

pub fn routes(files_path: PathBuf) -> Router {
    let state = AppState {
        files_path: Arc::new(files_path),
    };

    let route_static = r#static::routes(state.clone());

    let routes_v0 = v0::routes(state.clone());

    let route_openapi = SwaggerUi::new("/swagger-ui")
        .url("/api-docs/v0/openapi.json", v0::ApiDoc::merge_docs())
        .url(
            "/api-docs/static/openapi.json",
            r#static::ApiDoc::merge_docs(),
        );

    // Combine routes
    let app = routes_v0
        .merge(route_static)
        .merge(route_openapi)
        // IMPORTANT: It also swaps the type from Route<AppState> to Route
        // don't know the reason of swap but I guess the return signature forcing that swap...
        .with_state(state);

    // Create a CORS layer that allows all origins, methods, and headers
    let cors = CorsLayer::new()
        .allow_origin(Any) // Allow all origins
        .allow_methods(Any) // Allow all HTTP methods
        .allow_headers(Any); // Allow all headers

    // Layer the CORS, tracing middleware, and compression on top of the routes
    app.layer(axum::middleware::from_fn(
        middlewares::tracing::tracing_middleware,
    ))
    .layer(cors)
    .layer(CompressionLayer::new())
}
