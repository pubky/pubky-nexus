use crate::routes::v0::endpoints::{TAGS_HOT_ROUTE, TAG_ROUTE, TAG_TAGGERS_ROUTE};
use crate::routes::AppState;
use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

pub mod global;
pub mod view;

pub fn expensive_routes() -> Router<AppState> {
    Router::new().route(TAGS_HOT_ROUTE, get(global::hot_tags_handler))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(TAG_TAGGERS_ROUTE, get(global::tag_taggers_handler))
        .route(TAG_ROUTE, get(view::tag_view_handler))
}

#[derive(OpenApi)]
#[openapi()]
pub struct TagApiDoc;

impl TagApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        let mut combined = global::TagGlobalApiDoc::openapi();
        combined.merge(view::TagViewApiDoc::openapi());
        combined
    }
}
