use crate::routes::v0::endpoints::{
    SEARCH_POSTS_BY_CONTENT_ROUTE, SEARCH_POSTS_BY_TAG_ROUTE, SEARCH_TAGS_BY_PREFIX_ROUTE,
    SEARCH_USERS_BY_ID_ROUTE, SEARCH_USERS_BY_NAME_ROUTE, SEARCH_USERS_BY_TAGS_ROUTE,
};
use crate::routes::v0::version::ApiVersion;
use crate::routes::AppState;
use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

pub mod posts;
pub mod tags;
pub mod users;

pub use crate::models::user_id_prefix::USER_ID_SEARCH_MIN_PREFIX_LEN;

/// The expensive search routes for one version: the content search with the version's kind
/// filter, the user search by tags with the version's label list.
pub fn expensive_routes<V: ApiVersion>() -> Router<AppState> {
    Router::new()
        .route(SEARCH_POSTS_BY_CONTENT_ROUTE, V::search_posts_by_content())
        // Multi-label requests aggregate in the graph, so the route starts in
        // the expensive tier until production metrics justify loosening it
        .route(SEARCH_USERS_BY_TAGS_ROUTE, V::search_users_by_tags())
}

/// The default-bucket search routes for one version: the post search by tag and the tag
/// search by prefix with the version's label rule, the rest shared.
pub fn routes<V: ApiVersion>() -> Router<AppState> {
    Router::new()
        .route(
            SEARCH_USERS_BY_NAME_ROUTE,
            get(users::search_users_by_name_handler),
        )
        .route(
            SEARCH_USERS_BY_ID_ROUTE,
            get(users::search_users_by_id_handler),
        )
        // A reach turns this into the same graph traversal `stream/posts`
        // runs, and that route is in the default bucket too; bucket them
        // together if the traversal proves costly
        .route(SEARCH_POSTS_BY_TAG_ROUTE, V::search_posts_by_tag())
        .route(SEARCH_TAGS_BY_PREFIX_ROUTE, V::search_tags_by_prefix())
}

#[derive(OpenApi)]
#[openapi()]
pub struct SearchApiDoc;

impl SearchApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        let mut combined = users::SearchUsersApiDocs::openapi();
        combined.merge(posts::SearchPostsApiDocs::openapi());
        combined
    }
}
