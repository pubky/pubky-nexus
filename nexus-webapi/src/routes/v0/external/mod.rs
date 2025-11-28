use crate::routes::v0::endpoints::{EXTERNAL_TAGGERS_ROUTE, EXTERNAL_TAGS_ROUTE};
use crate::routes::AppState;
use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

pub mod tags;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(EXTERNAL_TAGS_ROUTE, get(tags::external_tags_handler))
        .route(EXTERNAL_TAGGERS_ROUTE, get(tags::external_taggers_handler))
}

#[derive(OpenApi)]
#[openapi()]
pub struct ExternalTagApiDoc;

impl ExternalTagApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        tags::ExternalTagsApiDoc::openapi()
    }
}
