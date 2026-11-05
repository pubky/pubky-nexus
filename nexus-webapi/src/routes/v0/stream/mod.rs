use crate::routes::v0::endpoints::{
    STREAM_POSTS_BY_IDS_ROUTE, STREAM_POSTS_ROUTE, STREAM_POST_KEYS_ROUTE, STREAM_RESOURCES_ROUTE,
    STREAM_RESOURCE_IDS_ROUTE, STREAM_USERS_BY_IDS_ROUTE, STREAM_USERS_ROUTE,
    STREAM_USERS_USERNAME_SEARCH_ROUTE, STREAM_USER_IDS_ROUTE,
};
use crate::routes::v0::version::ApiVersion;
use crate::routes::AppState;

use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

pub mod posts;
mod resources;
pub mod users;

/// The expensive stream routes for one version: the user streams and the `by_ids` post stream
/// in the version's shape.
pub fn expensive_routes<V: ApiVersion>() -> Router<AppState> {
    Router::new()
        .route(STREAM_USERS_ROUTE, V::stream_users())
        .route(STREAM_USERS_BY_IDS_ROUTE, V::stream_users_by_ids())
        .route(STREAM_POSTS_BY_IDS_ROUTE, V::stream_posts_by_ids())
}

/// The default-bucket stream routes for one version: the username search and the post stream
/// in the version's shape, it and the post keys with the version's kind filters, the rest
/// shared.
pub fn routes<V: ApiVersion>() -> Router<AppState> {
    Router::new()
        .route(STREAM_USER_IDS_ROUTE, get(users::stream_user_ids_handler))
        .route(
            STREAM_USERS_USERNAME_SEARCH_ROUTE,
            V::stream_username_search(),
        )
        .route(STREAM_POST_KEYS_ROUTE, V::stream_post_keys())
        .route(STREAM_POSTS_ROUTE, V::stream_posts())
        .route(
            STREAM_RESOURCES_ROUTE,
            get(resources::stream_resources_handler),
        )
        .route(
            STREAM_RESOURCE_IDS_ROUTE,
            get(resources::stream_resource_ids_handler),
        )
}

#[derive(OpenApi)]
#[openapi()]
pub struct StreamApiDoc;

impl StreamApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        let mut combined = users::StreamUsersApiDocs::openapi();
        combined.merge(posts::StreamPostsApiDocs::openapi());
        combined.merge(resources::StreamResourcesApiDocs::openapi());
        combined
    }
}
