use crate::register_routes;
use crate::routes::v0::endpoints;
use axum::Router;
use utoipa::OpenApi;

mod search;
mod trending;

pub fn routes() -> Router {
    register_routes!(Router::new(),
        endpoints::TAG_SEARCH_ROUTE => search::search_tags_handler,
        endpoints::TAG_TRENDING_ROUTE => trending::trending_tags_handler
    )
}

#[derive(OpenApi)]
#[openapi()]
pub struct ProfileApiDoc;

impl ProfileApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        search::SearchTagApiDoc::openapi()
            .merge(trending::TrendingTagsApiDoc::openapi())
    }
}
