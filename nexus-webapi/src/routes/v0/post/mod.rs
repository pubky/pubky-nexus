use crate::routes::v0::endpoints::{
    POST_BOOKMARK_ROUTE, POST_COUNTS_ROUTE, POST_DETAILS_ROUTE, POST_ROUTE, POST_TAGGERS_ROUTE,
    POST_TAGS_ROUTE,
};
use crate::routes::AppState;
use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

mod bookmark;
mod counts;
mod details;
pub mod tags;
mod view;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(POST_ROUTE, get(view::post_view_handler))
        .route(POST_DETAILS_ROUTE, get(details::post_details_handler))
        .route(POST_COUNTS_ROUTE, get(counts::post_counts_handler))
        .route(POST_BOOKMARK_ROUTE, get(bookmark::post_bookmark_handler))
        .route(POST_TAGS_ROUTE, get(tags::post_tags_handler))
        .route(POST_TAGGERS_ROUTE, get(tags::post_taggers_handler))
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
        combined.merge(tags::PostTagsApiDoc::openapi());
        combined
    }
}
