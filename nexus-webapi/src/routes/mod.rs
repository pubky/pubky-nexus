use axum::body::Body;
use axum::extract::{DefaultBodyLimit, FromRequest, FromRequestParts};
use axum::http::request::Parts;
use axum::http::{Request, StatusCode};
use axum::Json as AxumJson;
use axum::Router;
use std::time::Duration;
use std::{path::PathBuf, sync::Arc};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::timeout::TimeoutLayer;
use utoipa_swagger_ui::SwaggerUi;

pub mod r#static;
pub mod v0;

mod middlewares;

/// JSON extractor that maps Axum rejections to `Error::InvalidInput`.
pub struct Json<T>(pub T);

impl<S, T> FromRequest<S> for Json<T>
where
    S: Send + Sync,
    T: serde::de::DeserializeOwned,
{
    type Rejection = crate::Error;

    async fn from_request(req: Request<Body>, state: &S) -> Result<Self, Self::Rejection> {
        let json: AxumJson<T> = AxumJson::from_request(req, state)
            .await
            .map_err(|rejection| crate::Error::invalid_input(rejection.to_string()))?;
        Ok(Json(json.0))
    }
}

/// Path extractor that maps Axum rejections to `Error::InvalidInput`.
pub struct Path<T>(pub T);

impl<S, T> FromRequestParts<S> for Path<T>
where
    S: Send + Sync,
    T: serde::de::DeserializeOwned + Send,
{
    type Rejection = crate::Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let path: axum::extract::Path<T> = axum::extract::Path::from_request_parts(parts, state)
            .await
            .map_err(|rejection| crate::Error::invalid_input(rejection.to_string()))?;
        Ok(Path(path.0))
    }
}

/// Query extractor that maps Axum rejections to `Error::InvalidInput`.
pub struct Query<T>(pub T);

impl<S, T> FromRequestParts<S> for Query<T>
where
    S: Send + Sync,
    T: serde::de::DeserializeOwned + Send,
{
    type Rejection = crate::Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let query: axum::extract::Query<T> = axum::extract::Query::from_request_parts(parts, state)
            .await
            .map_err(|rejection| crate::Error::invalid_input(rejection.to_string()))?;
        Ok(Query(query.0))
    }
}

#[derive(Clone)]
pub struct AppState {
    pub files_path: Arc<PathBuf>,
}

/// The application's routes: v0 API, static file serving, and OpenAPI/Swagger UI docs.
pub fn app_routes(state: AppState) -> Router<AppState> {
    let route_static = r#static::router(state.clone());

    let routes_v0 = v0::router(state.clone());

    let route_openapi = SwaggerUi::new("/swagger-ui")
        .url("/api-docs/v0/openapi.json", v0::ApiDoc::merge_docs())
        .url(
            "/api-docs/static/openapi.json",
            r#static::ApiDoc::merge_docs(),
        );

    routes_v0.merge(route_static).merge(route_openapi)
}

/// Builds the full application [Router]: attaches `routes` to `state`, then layers on
/// tracing, CORS, compression, request body size limit, and request timeout middleware.
pub fn build_app(
    routes: Router<AppState>,
    state: AppState,
    request_timeout_secs: u64,
    max_body_size_bytes: usize,
) -> Router {
    // with_state resolves the AppState generic, turning Router<AppState> into Router (= Router<()>)
    let app = routes.with_state(state);

    // Create a CORS layer that allows all origins, methods, and headers
    let cors = CorsLayer::new()
        .allow_origin(Any) // Allow all origins
        .allow_methods(Any) // Allow all HTTP methods
        .allow_headers(Any); // Allow all headers

    // Layer the request limits innermost, so that tracing and CORS still apply to the
    // 408/413 responses they short-circuit with (bypassing the rest of the stack).
    app.layer(CompressionLayer::new())
        .layer(RequestBodyLimitLayer::new(max_body_size_bytes))
        // Also raise the extractor limit (Json, Bytes, etc.), which otherwise defaults to
        // 2MB regardless of RequestBodyLimitLayer above, capping requests below max_body_size_bytes.
        .layer(DefaultBodyLimit::max(max_body_size_bytes))
        // Clamp to 1 s minimum: a zero-duration timeout fires before any handler runs,
        // returning 408 for every request. Treat 0 as "use the minimum".
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(request_timeout_secs.max(1)),
        ))
        .layer(cors)
        .layer(axum::middleware::from_fn(
            middlewares::tracing::tracing_middleware,
        ))
}
