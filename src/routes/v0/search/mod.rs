use crate::register_routes;
use crate::routes::v0::endpoints;
use axum::Router;
use utoipa::OpenApi;

mod users;

pub fn routes() -> Router {
    register_routes!(Router::new(),
        endpoints::SEARCH_USERS_ROUTE => users::search_users_handler
    )
}

#[derive(OpenApi)]
#[openapi()]
pub struct SearchApiDoc;

impl SearchApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        
        users::SearchUsersApiDocs::openapi()
    }
}
