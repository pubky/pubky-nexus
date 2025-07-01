use crate::Result;
use axum::{
    extract::Request,
    response::{IntoResponse, Redirect, Response},
};
use nexus_common::media::FileVariant;
use utoipa::OpenApi;

use super::endpoints::LEGACY_STATIC_FILES_ROUTE;

/// Handler to redirect legacy static files
/// The path should be in the format /static/{owner_id}/{file_id}
#[utoipa::path(
    get,
    path = LEGACY_STATIC_FILES_ROUTE,
    description = "Redirects to the new static file path",
    tag = "File",
    params(
        ("owner_id" = String, Path, description = "File's owner id"),
        ("file_id" = String, Path, description = "File's id"),
    ),
    responses(
        (status = 200, description = "File's raw data"),
        (status = 404, description = "File not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn legacy_files_handler(request: Request) -> Result<Response> {
    // Construct the new path
    let new_path = format!("{}/{}", request.uri().path(), FileVariant::Main);

    // Perform a redirect to the new path
    Ok(Redirect::permanent(&new_path).into_response())
}

#[derive(OpenApi)]
#[openapi(paths(legacy_files_handler))]
pub struct LegacyStaticFileApiDoc;
