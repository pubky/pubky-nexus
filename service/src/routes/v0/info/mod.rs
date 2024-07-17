use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;
use utoipa::{OpenApi, ToSchema};
use super::endpoints::INFO_PATH;

#[derive(Serialize, ToSchema)]
pub struct ServerInfo {
    description: String,
    homepage: String,
    license: String,
    name: String,
    repository: String,
    version: String,
}

#[utoipa::path(
    get,
    path = INFO_PATH,
    responses(
        (status = 200, description = "Server info", body = ServerInfo)
    )
)]
pub async fn info_handler() -> impl IntoResponse {
    let info = ServerInfo {
        description: env!("CARGO_PKG_DESCRIPTION").to_string(),
        homepage: env!("CARGO_PKG_HOMEPAGE").to_string(),
        license: env!("CARGO_PKG_LICENSE").to_string(),
        name: env!("CARGO_PKG_NAME").to_string(),
        repository: env!("CARGO_PKG_REPOSITORY").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    Json(info)
}

#[derive(OpenApi)]
#[openapi(paths(info_handler), components(schemas(ServerInfo)))]
pub struct ApiDoc;
