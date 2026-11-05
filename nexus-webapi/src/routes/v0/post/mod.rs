use crate::routes::v0::endpoints::{
    POST_BOOKMARK_ROUTE, POST_COUNTS_ROUTE, POST_DETAILS_ROUTE, POST_ROUTE, POST_TAGGERS_ROUTE,
    POST_TAGS_ROUTE,
};
use crate::routes::v0::version::ApiVersion;
use crate::routes::AppState;
use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

mod bookmark;
mod counts;
pub mod details;
pub mod tags;
pub mod view;

/// The post routes for one version: the view and the details in the version's shape, the
/// rest shared.
pub fn routes<V: ApiVersion>() -> Router<AppState> {
    Router::new()
        .route(POST_ROUTE, V::post_view())
        .route(POST_DETAILS_ROUTE, V::post_details())
        .route(POST_COUNTS_ROUTE, get(counts::post_counts_handler))
        .route(POST_BOOKMARK_ROUTE, get(bookmark::post_bookmark_handler))
        .route(POST_TAGS_ROUTE, get(tags::post_tags_handler))
        .route(POST_TAGGERS_ROUTE, get(tags::post_taggers_handler))
}

#[derive(OpenApi)]
#[openapi()]
pub struct PostApiDoc;

impl PostApiDoc {
    /// The shared post routes and schemas; the versioned view and details come with the
    /// version's document.
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        let mut combined = view::PostViewApiDoc::openapi();
        combined.merge(counts::PostCountsApiDoc::openapi());
        combined.merge(bookmark::BookmarkApiDoc::openapi());
        combined.merge(details::PostDetailsApiDoc::openapi());
        combined.merge(tags::PostTagsApiDoc::openapi());
        combined
    }
}
