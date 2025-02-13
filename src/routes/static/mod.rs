use crate::{register_routes, Config};
use axum::{routing::get_service, Router};
use endpoints::{LEGACY_STATIC_FILES_ROUTE, STATIC_FILES_ROUTE, STATIC_ROUTE};
use files::static_files_handler;
use legacy_files::legacy_files_handler;
use tower_http::services::ServeDir;
use utoipa::OpenApi;

mod endpoints;
mod files;
mod legacy_files;
mod serve_dir;

pub use serve_dir::get_serve_dir;

pub fn routes() -> Router {
    let config = Config::from_env();

    let router = register_routes!(
        Router::new(),
        STATIC_FILES_ROUTE => static_files_handler,
        LEGACY_STATIC_FILES_ROUTE => legacy_files_handler
    );

    router.nest_service(STATIC_ROUTE, get_service(ServeDir::new(config.static_path)))
}

#[derive(OpenApi)]
#[openapi()]
pub struct ApiDoc;

impl ApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        let mut combined = files::StaticFileApiDoc::openapi();
        combined.merge(legacy_files::LegacyStaticFileApiDoc::openapi());
        combined
    }
}
