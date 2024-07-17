use axum::routing::get;
use axum::Router;
use endpoints::{INFO_ROUTE, PROFILE_ROUTE};
use pk_social_common::connectors::neo4j::Neo4jConnector;
use std::sync::Arc;
use utoipa::OpenApi;

use crate::models::{info::ServerInfo, profile::Profile};

pub mod endpoints;
pub mod info;
pub mod profile;

pub fn routes(neo4j_connector: Arc<Neo4jConnector>) -> Router {
    Router::new()
        .route(INFO_ROUTE, get(info::info_handler))
        .route(
            &format!("{}/:user_id", PROFILE_ROUTE),
            get(move |path| profile::get_profile(path, neo4j_connector.clone())),
        )
}

#[derive(OpenApi)]
#[openapi(
    paths(info::info_handler, profile::get_profile),
    components(schemas(ServerInfo, Profile))
)]
pub struct ApiDoc;
