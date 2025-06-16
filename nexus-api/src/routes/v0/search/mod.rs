use crate::register_routes;
use crate::routes::v0::endpoints;
use crate::routes::AppState;
use axum::Router;
use utoipa::OpenApi;

mod posts;
mod tags;
mod users;

pub const USER_ID_SEARCH_MIN_PREFIX_LEN: usize = 3;

pub fn routes() -> Router<AppState> {
    register_routes!(Router::new(),
        endpoints::SEARCH_USERS_BY_NAME_ROUTE => users::search_users_by_name_handler,
        endpoints::SEARCH_USERS_BY_ID_ROUTE=> users::search_users_by_id_handler,
        endpoints::SEARCH_POSTS_BY_TAG_ROUTE => posts::search_posts_by_tag_handler,
        endpoints::SEARCH_TAGS_BY_PREFIX_ROUTE => tags::search_tags_by_prefix_handler
    )
}

#[derive(OpenApi)]
#[openapi()]
pub struct SearchApiDoc;

impl SearchApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        let mut combined = users::SearchUsersApiDocs::openapi();
        combined.merge(posts::SearchPostsByTagApiDocs::openapi());
        combined.merge(tags::SearchTagsByPrefixApiDocs::openapi());
        combined
    }
}
