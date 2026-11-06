use super::AppState;

use crate::models::PubkyId;
use axum::{routing::get, Router};
use endpoints::{
    LEGACY_STATIC_FILES_ROUTE, STATIC_FILES_ROUTE, USER_AVATAR_ROUTE, V1_STATIC_FILES_ROUTE,
    V1_USER_AVATAR_ROUTE,
};
use nexus_common::media::FileVariant;
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
        .route(V1_STATIC_FILES_ROUTE, get(files::v1_static_files_handler))
        .route(USER_AVATAR_ROUTE, get(avatar::user_avatar_handler))
        .route(V1_USER_AVATAR_ROUTE, get(avatar::v1_user_avatar_handler));

    let default = Router::new().route(
        LEGACY_STATIC_FILES_ROUTE,
        get(legacy_files::legacy_files_handler),
    );

    (expensive, default)
}

/// The static document: the `/static` routes, which go with `/v0`. The two `/v1/static`
/// routes this router also serves are documented with `/v1` ([`V1StaticApiDoc`]).
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

/// The two `/v1/static` routes, the file bytes and the avatar under the v1 base path. They are
/// served here, at their absolute paths, and documented in the `/v1` document
/// (`V1::absolute_docs`), so one generated client covers everything a v1 client calls.
#[derive(OpenApi)]
#[openapi(
    paths(files::v1_static_files_handler, avatar::v1_user_avatar_handler),
    components(schemas(FileVariant, PubkyId))
)]
pub struct V1StaticApiDoc;
