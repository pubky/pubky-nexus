use super::AppState;

use axum::{routing::get, Router};
use endpoints::{LEGACY_STATIC_FILES_ROUTE, STATIC_FILES_ROUTE, USER_AVATAR_ROUTE};
use utoipa::OpenApi;

pub use serve_dir::PubkyServeDir;

mod avatar;
mod endpoints;
mod files;
mod legacy_files;
mod serve_dir;

pub fn routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        .with_state(app_state)
        .route(STATIC_FILES_ROUTE, get(files::static_files_handler))
        .route(
            LEGACY_STATIC_FILES_ROUTE,
            get(legacy_files::legacy_files_handler),
        )
        .route(USER_AVATAR_ROUTE, get(avatar::user_avatar_handler))
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
