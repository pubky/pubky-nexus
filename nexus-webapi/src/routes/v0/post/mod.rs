use crate::routes::v0::endpoints::{
    POST_COUNTS_ROUTE, POST_DETAILS_ROUTE, POST_ROUTE, POST_TAGGERS_ROUTE, POST_TAGS_ROUTE,
};
use crate::routes::v0::version::ApiVersion;
use crate::routes::AppState;
use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

pub mod bookmark;
mod counts;
pub mod details;
pub mod tags;
pub mod view;

/// The post routes every version serves: the view, the details, the tags and the taggers in
/// the version's shape, the taggers with the version's label rule, the counts shared. The
/// bookmark route is `/v0`'s alone, listed with the version.
pub fn routes<V: ApiVersion>() -> Router<AppState> {
    Router::new()
        .route(POST_ROUTE, V::post_view())
        .route(POST_DETAILS_ROUTE, V::post_details())
        .route(POST_COUNTS_ROUTE, get(counts::post_counts_handler))
        .route(POST_TAGS_ROUTE, V::post_tags())
        .route(POST_TAGGERS_ROUTE, V::post_taggers())
}

#[derive(OpenApi)]
#[openapi()]
pub struct PostApiDoc;

impl PostApiDoc {
    /// The shared post routes and schemas; the versioned view, details, tags and taggers, and
    /// `/v0`'s bookmark route, come with the version's document.
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        let mut combined = view::PostViewApiDoc::openapi();
        combined.merge(counts::PostCountsApiDoc::openapi());
        combined.merge(details::PostDetailsApiDoc::openapi());
        combined.merge(tags::PostTagsApiDoc::openapi());
        combined
    }
}
