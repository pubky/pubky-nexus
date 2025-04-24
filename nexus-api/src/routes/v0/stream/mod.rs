use crate::register_routes;
use crate::routes::v0::endpoints;
use crate::routes::AppState;
use axum::Router;
use utoipa::OpenApi;

mod posts;
mod users;

pub fn routes() -> Router<AppState> {
    let router = register_routes!(Router::new(),
        // User stream
        endpoints::STREAM_USERS_ROUTE => users::stream_users_handler,
        endpoints::STREAM_USERS_USERNAME_SEARCH_ROUTE => users::stream_username_search_handler,

        // Post stream
        endpoints::STREAM_POSTS_ROUTE => posts::stream_posts_handler
    );

    // Register the POST route separately
    router
        .route(
            endpoints::STREAM_USERS_BY_IDS_ROUTE,
            axum::routing::post(users::stream_users_by_ids_handler),
        )
        .route(
            endpoints::STREAM_POSTS_BY_IDS_ROUTE,
            axum::routing::post(posts::stream_posts_by_ids_handler),
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
