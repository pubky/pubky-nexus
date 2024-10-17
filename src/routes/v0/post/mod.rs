use crate::routes::v0::endpoints;
use crate::{register_routes, to_axum};
use axum::Router;
use utoipa::OpenApi;

mod bookmark;
mod counts;
mod details;
mod tags;
mod thread;
mod view;
//mod replies;

pub fn routes() -> Router {
    register_routes!(Router::new(),
        to_axum!(endpoints::POST_ROUTE) => view::post_view_handler,
        to_axum!(endpoints::POST_DETAILS_ROUTE) => details::post_details_handler,
        to_axum!(endpoints::POST_COUNTS_ROUTE) => counts::post_counts_handler,
        to_axum!(endpoints::POST_BOOKMARK_ROUTE) => bookmark::post_bookmark_handler,
        to_axum!(endpoints::THREAD_ROUTE) => thread::thread_handler,
        to_axum!(endpoints::POST_TAGS_ROUTE) => tags::post_tags_handler,
        to_axum!(endpoints::POST_TAGGERS_ROUTE) => tags::post_taggers_handler,
        to_axum!(endpoints::POST_REPLIES_ROUTE) => tags::post_taggers_handler,
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
