use crate::routes::v0::endpoints;
use crate::{register_routes, to_axum};
use axum::Router;
use utoipa::OpenApi;

mod tags;
mod users;

pub fn routes() -> Router {
    register_routes!(Router::new(),
        to_axum!(endpoints::SEARCH_USERS_ROUTE) => users::search_users_handler,
        to_axum!(endpoints::SEARCH_TAGS_ROUTE) => tags::search_post_tags_handler
    )
}

#[derive(OpenApi)]
#[openapi()]
pub struct SearchApiDoc;

impl SearchApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        let mut combined = users::SearchUsersApiDocs::openapi();
        combined.merge(tags::SearchTagPostsApiDocs::openapi());
        combined
    }
}
