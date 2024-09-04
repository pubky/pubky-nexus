use crate::register_routes;
use crate::routes::v0::endpoints;
use axum::Router;
use utoipa::OpenApi;

mod posts;
mod users;

pub fn routes() -> Router {
    register_routes!(Router::new(),
        //  User stream
        endpoints::STREAM_USERS_ROUTE => users::stream_users_handler,
        endpoints::STREAM_USERS_USERNAME_SEARCH_ROUTE => users::stream_username_search_handler,
        endpoints::STREAM_USERS_MOSTFOLLOWED_ROUTE => users::stream_most_followed_users_handler,
        endpoints::STREAM_USERS_PIONEERS_ROUTE => users::stream_pioneer_users_handler,
        // Post stream
        endpoints::STREAM_POSTS_ROUTE => posts::stream_global_posts_handler,
        endpoints::STREAM_POSTS_USER_ROUTE => posts::stream_user_posts_handler,
        endpoints::STREAM_POSTS_REACH_ROUTE => posts::stream_posts_by_reach_handler,
        endpoints::STREAM_POSTS_BOOKMARKED_ROUTE => posts::stream_bookmarked_posts_handler,
        endpoints::STREAM_POSTS_TAG_ROUTE => posts::stream_posts_by_tags_handler
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
