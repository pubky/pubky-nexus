use super::AppState;
use crate::register_routes_with_state;
use axum::Router;
use endpoints::{LEGACY_STATIC_FILES_ROUTE, STATIC_FILES_ROUTE, USER_AVATAR_ROUTE};
use utoipa::OpenApi;

mod avatar;
mod endpoints;
mod files;
mod legacy_files;
mod serve_dir;
pub use serve_dir::PubkyServeDir;

pub fn routes(app_state: AppState) -> Router<AppState> {
    register_routes_with_state!(
        Router::new(),
        app_state,
        STATIC_FILES_ROUTE => files::static_files_handler,
        LEGACY_STATIC_FILES_ROUTE => legacy_files::legacy_files_handler,
        USER_AVATAR_ROUTE => avatar::user_avatar_handler,
    )
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
