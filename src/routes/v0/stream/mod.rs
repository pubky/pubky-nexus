use crate::register_routes;
use crate::routes::v0::endpoints;
use axum::Router;
use utoipa::OpenApi;

mod posts;
mod users;

pub fn routes() -> Router {
    register_routes!(Router::new(),
        endpoints::STREAM_USERS_ROUTE => users::stream_users_handler,
        endpoints::STREAM_POSTS_ROUTE => posts::stream_global_posts_handler,
        endpoints::STREAM_POSTS_USER_ROUTE => posts::stream_user_posts_handler,
        endpoints::STREAM_POSTS_REACH_ROUTE => posts::stream_posts_by_reach_handler,
    )
}

#[derive(OpenApi)]
#[openapi()]
pub struct StreamApiDoc;

impl StreamApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        users::StreamUsersApiDocs::openapi();
        posts::StreamPostsApiDocs::openapi()
    }
}
