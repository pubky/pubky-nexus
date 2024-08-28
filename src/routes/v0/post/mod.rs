use crate::register_routes;
use crate::routes::v0::endpoints;
use axum::Router;
use utoipa::OpenApi;

mod bookmark;
mod counts;
mod details;
mod tags;
mod thread;
mod view;

pub fn routes() -> Router {
    register_routes!(Router::new(),
        endpoints::POST_ROUTE => view::post_view_handler,
        endpoints::POST_DETAILS_ROUTE => details::post_details_handler,
        endpoints::POST_COUNTS_ROUTE => counts::post_counts_handler,
        endpoints::POST_BOOKMARK_ROUTE => bookmark::post_bookmark_handler,
        endpoints::THREAD_ROUTE => thread::thread_handler,
        endpoints::POST_TAGS_ROUTE => tags::post_tags_handler
    )
}

#[derive(OpenApi)]
#[openapi()]
pub struct PostApiDoc;

impl PostApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        let mut combined = view::PostViewApiDoc::openapi();
        combined.merge(counts::PostCountsApiDoc::openapi());
        combined.merge(bookmark::BookmarkApiDoc::openapi());
        combined.merge(details::PostDetailsApiDoc::openapi());
        combined.merge(thread::ThreadViewApiDoc::openapi());
        combined.merge(tags::PostTagsApiDoc::openapi());
        combined
    }
}
