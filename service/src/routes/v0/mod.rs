use axum::routing::get;
use axum::Router;
use endpoints::{INFO_ROUTE, PROFILE_ROUTE};
use utoipa::OpenApi;

use crate::models::{info::ServerInfo, profile::Profile};

pub mod endpoints;
pub mod info;
pub mod profile;

pub fn routes() -> Router {
    Router::new()
        .route(INFO_ROUTE, get(info::info_handler))
        .route(
            &format!("{}/:user_id", PROFILE_ROUTE),
            get(profile::get_profile),
        )
}

#[derive(OpenApi)]
#[openapi(
    paths(info::info_handler, profile::get_profile),
    components(schemas(ServerInfo, Profile))
)]
pub struct ApiDoc;
