use crate::register_routes;
use crate::routes::v0::endpoints;
use axum::Router;
use utoipa::OpenApi;

mod counts;
mod details;
mod view;

pub fn routes() -> Router {
    register_routes!(Router::new(),
        endpoints::POST_ROUTE => view::post_view_handler,
        endpoints::POST_DETAILS_ROUTE => details::post_details_handler,
    )
}

#[derive(OpenApi)]
#[openapi()]
pub struct PostApiDoc;

impl PostApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        let mut combined = view::PostViewApiDoc::openapi();
        combined.merge(details::PostDetailsApiDoc::openapi());
        combined
    }
}
