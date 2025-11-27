use crate::routes::v0::endpoints::{
    STREAM_POSTS_BY_IDS_ROUTE, STREAM_POSTS_ROUTE, STREAM_POST_KEYS_ROUTE,
    STREAM_USERS_BY_IDS_ROUTE, STREAM_USERS_ROUTE, STREAM_USERS_USERNAME_SEARCH_ROUTE,
    STREAM_USER_IDS_ROUTE,
};
use crate::routes::AppState;

use axum::routing::{get, post};
use axum::Router;
use utoipa::OpenApi;

mod posts;
mod users;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(STREAM_USERS_ROUTE, get(users::stream_users_handler))
        .route(STREAM_USER_IDS_ROUTE, get(users::stream_user_ids_handler))
        .route(
            STREAM_USERS_USERNAME_SEARCH_ROUTE,
            get(users::stream_username_search_handler),
        )
        .route(STREAM_POST_KEYS_ROUTE, get(posts::stream_post_keys_handler))
        .route(STREAM_POSTS_ROUTE, get(posts::stream_posts_handler))
        .route(
            STREAM_USERS_BY_IDS_ROUTE,
            post(users::stream_users_by_ids_handler),
        )
        .route(
            STREAM_POSTS_BY_IDS_ROUTE,
            post(posts::stream_posts_by_ids_handler),
        )
}

#[derive(OpenApi)]
#[openapi()]
pub struct StreamApiDoc;

impl StreamApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        let mut combined = users::StreamUsersApiDocs::openapi();
        combined.merge(posts::StreamPostsApiDocs::openapi());
        combined
    }
}
