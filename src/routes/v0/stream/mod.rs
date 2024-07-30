use crate::register_routes;
use crate::routes::v0::endpoints;
use axum::Router;
use utoipa::OpenApi;

mod users;

pub fn routes() -> Router {
    register_routes!(Router::new(),
        endpoints::STREAM_USERS_ROUTE => users::stream_users_handler,
    )
}

#[derive(OpenApi)]
#[openapi()]
pub struct StreamApiDoc;

impl StreamApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        
        users::StreamUsersApiDocs::openapi()
    }
}
