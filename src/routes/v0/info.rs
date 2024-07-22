use super::endpoints::INFO_ROUTE;
use crate::models::info::ServerInfo;
use crate::register_routes;
use axum::response::IntoResponse;
use axum::{Json, Router};
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = INFO_ROUTE,
    tag = "Service Info",
    responses(
        (status = 200, description = "Server info", body = ServerInfo)
    )
)]
pub async fn info_handler() -> impl IntoResponse {
    let info = ServerInfo::new();
    Json(info)
}

pub fn routes() -> Router {
    register_routes!(Router::new(), super::endpoints::INFO_ROUTE => info_handler)
}

#[derive(OpenApi)]
#[openapi(paths(info_handler), components(schemas(ServerInfo)))]
pub struct InfoApiDoc;
