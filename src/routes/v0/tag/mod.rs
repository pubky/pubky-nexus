use crate::routes::v0::endpoints;
use crate::{register_routes, to_axum};
use axum::Router;
use utoipa::OpenApi;

mod global;

pub fn routes() -> Router {
    register_routes!(Router::new(),
        to_axum!(endpoints::TAG_HOT_ROUTE) => global::hot_tags_handler,
        to_axum!(endpoints::TAG_REACH_ROUTE) => global::tags_by_reach_handler,
        to_axum!(endpoints::TAG_TAGGERS_ROUTE) => global::tag_taggers_handler
    )
}

#[derive(OpenApi)]
#[openapi()]
pub struct TagApiDoc;

impl TagApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        global::TagGlobalApiDoc::openapi()
    }
}
