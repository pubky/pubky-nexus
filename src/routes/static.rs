use crate::{
    static_processor::{legacy_static_files_middleware, static_files_middleware},
    to_axum, Config,
};
use axum::{
    middleware::{self},
    routing::get_service,
    Router,
};
use tower_http::services::ServeDir;

const STATIC_ROUTE: &str = "/static";
const STATIC_FILES_ROUTE: &str = "/static/files/{owner_id}/{file_id}/{variant}";
const LEGACY_STATIC_FILES_ROUTE: &str = "/static/files/{owner_id}/{file_id}";

pub fn routes() -> Router {
    let config = Config::from_env();

    let general =
        Router::new().nest_service(STATIC_ROUTE, get_service(ServeDir::new(config.static_path)));

    let files = Router::new()
        .nest_service(
            to_axum!(STATIC_FILES_ROUTE),
            get_service(ServeDir::new(config.file_path.clone())),
        )
        .route_layer(middleware::from_fn(static_files_middleware));

    let legacy_files = Router::new()
        .nest_service(
            to_axum!(LEGACY_STATIC_FILES_ROUTE),
            get_service(ServeDir::new(config.file_path)),
        )
        .route_layer(middleware::from_fn(legacy_static_files_middleware));

    general.merge(files).merge(legacy_files)
}
