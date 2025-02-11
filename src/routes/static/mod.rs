use crate::{register_routes, Config};
use axum::{routing::get_service, Router};
use endpoints::{LEGACY_STATIC_FILES_ROUTE, STATIC_FILES_ROUTE, STATIC_ROUTE};
use files::static_files_handler;
use legacy_files::legacy_files_handler;
use once_cell::sync::OnceCell;
use tower_http::services::ServeDir;

mod endpoints;
mod files;
mod legacy_files;

pub fn routes() -> Router {
    let config = Config::from_env();

    let router = register_routes!(
        Router::new(),
        STATIC_FILES_ROUTE => static_files_handler,
        LEGACY_STATIC_FILES_ROUTE => legacy_files_handler
    );

    router.nest_service(STATIC_ROUTE, get_service(ServeDir::new(config.static_path)))
}

static SERVE_DIR_INSTANCE: OnceCell<ServeDir> = OnceCell::new();

pub fn get_serve_dir() -> ServeDir {
    SERVE_DIR_INSTANCE
        .get_or_init(|| {
            let config = Config::from_env();
            ServeDir::new(config.file_path)
        })
        .to_owned()
}
