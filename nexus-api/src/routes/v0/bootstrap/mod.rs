use crate::register_routes;
use crate::routes::v0::endpoints;
use crate::routes::AppState;
use axum::Router;
use utoipa::OpenApi;

mod im_alive;

pub fn routes() -> Router<AppState> {
    register_routes!(Router::new(),
        endpoints::IM_ALIVE_ROUTE => im_alive::user_bootstrap_handler,
    )
}

#[derive(OpenApi)]
#[openapi()]
pub struct BootstrapApiDoc;

impl BootstrapApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        im_alive::ImAliveApiDoc::openapi()
    }
}
