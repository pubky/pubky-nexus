use super::AppState;

use axum::{routing::get, Router};
use endpoints::{LEGACY_STATIC_FILES_ROUTE, STATIC_FILES_ROUTE, USER_AVATAR_ROUTE};
use utoipa::OpenApi;

pub use avatar::user_avatar_handler;
pub use serve_dir::PubkyServeDir;

mod avatar;
mod endpoints;
mod files;
mod legacy_files;
mod serve_dir;

/// Returns (expensive_routes, default_routes).
/// Expensive routes spawn ImageMagick on first hit per variant.
pub fn routes() -> (Router<AppState>, Router<AppState>) {
    let expensive = Router::new()
        .route(STATIC_FILES_ROUTE, get(files::static_files_handler))
        .route(USER_AVATAR_ROUTE, get(avatar::user_avatar_handler));

    let default = Router::new().route(
        LEGACY_STATIC_FILES_ROUTE,
        get(legacy_files::legacy_files_handler),
    );

    (expensive, default)
}

#[derive(OpenApi)]
#[openapi()]
pub struct ApiDoc;

impl ApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        let mut combined = files::StaticFileApiDoc::openapi();
        combined.merge(legacy_files::LegacyStaticFileApiDoc::openapi());
        combined.merge(avatar::UserAvatarApiDoc::openapi());
        combined
    }
}
