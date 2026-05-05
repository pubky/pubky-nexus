use axum::body::Body;
use axum::extract::{FromRequest, FromRequestParts};
use axum::http::request::Parts;
use axum::http::Request;
use axum::Json as AxumJson;
use axum::Router;
use std::{path::PathBuf, sync::Arc};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use utoipa_swagger_ui::SwaggerUi;

pub mod r#static;
pub mod v0;

mod middlewares;

/// A wrapper around Axum's Json extractor that maps deserialization errors
/// to Error::InvalidInput (400 Bad Request) for consistent error handling
/// across all extraction types (Query, Path, and Json).
pub struct ValidJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidJson<T>
where
    S: Send + Sync,
    T: serde::de::DeserializeOwned,
{
    type Rejection = crate::Error;

    async fn from_request(req: Request<Body>, state: &S) -> Result<Self, Self::Rejection> {
        let json: AxumJson<T> = AxumJson::from_request(req, state)
            .await
            .map_err(|rejection| crate::Error::invalid_input(&rejection.to_string()))?;
        Ok(ValidJson(json.0))
    }
}

/// A wrapper around Axum's Path extractor that maps deserialization/validation errors
/// to Error::InvalidInput (400 Bad Request) for consistent JSON error responses.
/// This ensures that path parameter validation failures return a proper JSON body
/// instead of Axum's default plain-text rejection.
pub struct ValidPath<T>(pub T);

impl<S, T> FromRequestParts<S> for ValidPath<T>
where
    S: Send + Sync,
    T: serde::de::DeserializeOwned + std::str::FromStr + Send,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    type Rejection = crate::Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let path: axum::extract::Path<T> = axum::extract::Path::from_request_parts(parts, state)
            .await
            .map_err(|rejection| crate::Error::invalid_input(&rejection.to_string()))?;
        Ok(ValidPath(path.0))
    }
}

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
