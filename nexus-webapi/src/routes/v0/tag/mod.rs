use crate::register_routes;
use crate::routes::v0::endpoints;
use crate::routes::AppState;
use axum::Router;
use utoipa::OpenApi;

mod global;

pub fn routes() -> Router<AppState> {
    register_routes!(Router::new(),
        endpoints::TAGS_HOT_ROUTE => global::hot_tags_handler,
        endpoints::TAG_TAGGERS_ROUTE => global::tag_taggers_handler
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
