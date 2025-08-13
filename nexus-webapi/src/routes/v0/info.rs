use std::path::PathBuf;

use super::endpoints::INFO_ROUTE;
use crate::models::info::ServerInfo;
use crate::routes::AppState;

use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = INFO_ROUTE,
    tag = "Info",
    responses(
        (status = 200, description = "Server info", body = ServerInfo)
    )
)]
pub async fn info_handler(State(app_state): State<AppState>) -> impl IntoResponse {
    let file_path: &PathBuf = &app_state.files_path;
    let info = ServerInfo::new(file_path.clone()).await;
    Json(info)
}

pub fn routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        .with_state(app_state)
        .route(INFO_ROUTE, get(info_handler))
}

#[derive(OpenApi)]
#[openapi(paths(info_handler), components(schemas(ServerInfo)))]
pub struct InfoApiDoc;
