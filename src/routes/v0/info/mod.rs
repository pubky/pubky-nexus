use super::endpoints::INFO_ROUTE;
use crate::models::info::ServerInfo;
use axum::response::IntoResponse;
use axum::Json;

#[utoipa::path(
    get,
    path = INFO_ROUTE,
    responses(
        (status = 200, description = "Server info", body = ServerInfo)
    )
)]
pub async fn info_handler() -> impl IntoResponse {
    let info = ServerInfo::new();
    Json(info)
}
