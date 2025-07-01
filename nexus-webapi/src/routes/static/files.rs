use std::path::PathBuf;

use axum::{
    extract::{Path, Query, Request, State},
    response::Response,
};
use serde::{Deserialize, Serialize};
use tower_http::services::fs::ServeFileSystemResponseBody;
use tracing::{debug, error};
use utoipa::OpenApi;

use crate::routes::{r#static::PubkyServeDir, AppState};
use crate::{Error, Result};
use nexus_common::{
    media::{FileVariant, VariantController},
    models::{
        file::{Blob, FileDetails},
        traits::Collection,
    },
};

use super::endpoints::STATIC_FILES_ROUTE;

#[derive(Deserialize, Serialize)]
pub struct FileParams {
    // dl is download parameter to set the content-disposition header to attachment
    dl: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct FilePath {
    owner_id: String,
    file_id: String,
    variant: FileVariant,
}

/// Handler to serve static files
/// If the variant has not been created, it will be created on the fly
/// If the variant is not valid for the content type, a 400 Bad Request will be returned
/// If the file does not exist, a 404 Not Found will be returned
/// If the processing of the new variant fails, a 500 Internal Server Error will be returned
#[utoipa::path(
    get,
    path = STATIC_FILES_ROUTE,
    description = "Serves a static file by owner_id, file_id and variant",
    tag = "File",
    params(
        ("owner_id" = String, Path, description = "File's owner id"),
        ("file_id" = String, Path, description = "File's id"),
        ("variant" = FileVariant, Path, description = "File's variant"),
        ("dl" = Option<String>, Query, description = "Download the file. Returns with content-disposition header set to attachment")
    ),
    responses(
        (status = 200, description = "File's raw data"),
        (status = 404, description = "File not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn static_files_handler(
    Path(FilePath {
        owner_id,
        file_id,
        variant,
    }): Path<FilePath>,
    State(app_state): State<AppState>,
    params: Query<FileParams>,
    request: Request,
) -> Result<Response<ServeFileSystemResponseBody>> {
    debug!(
        "Serving file for user: {} and file: {} with variant: {:?}",
        owner_id, file_id, variant
    );

    let file_path: &PathBuf = &app_state.files_path;

    let files = FileDetails::get_by_ids(
        vec![vec![owner_id.as_str(), file_id.as_str()].as_slice()].as_slice(),
    )
    .await
    .map_err(|err| {
        error!(
            "Error while fetching file details for user: {} and file: {}",
            owner_id, file_id
        );
        Error::InternalServerError { source: err }
    })?;

    if files.is_empty() {
        return Err(Error::FileNotFound {});
    }

    let file = files
        .first()
        .and_then(Clone::clone)
        .ok_or(Error::FileNotFound {})?;

    if !VariantController::validate_variant_for_content_type(file.content_type.as_str(), &variant) {
        return Err(Error::InvalidInput {
            message: format!(
                "variant {} is not valid for content type {}",
                variant, file.content_type
            ),
        });
    }

    let file_variant_content_type = Blob::get_by_id(&file, &variant, file_path.clone())
        .await
        .map_err(|err| {
            error!(
                "Error while processing file variant for variant: {} and file: {}",
                variant, file_id
            );
            Error::InternalServerError { source: err }
        })?;

    let request_uri = request.uri().clone();

    let mut response = PubkyServeDir::try_call(
        request,
        request_uri.path().replace("static/files", ""),
        file_variant_content_type,
        file_path.clone(),
    )
    .await?;

    // Remove any default "cache-control" header (which may be set to no-cache)
    response.headers_mut().remove("cache-control");

    // Set a new Cache-Control header to cache the file for 3600 seconds (1 hour)
    let cache_control_header = "public, max-age=3600".parse().map_err(|err| {
        error!("Failed to parse Cache-Control header value: {}", err);
        Error::InternalServerError {
            source: Box::new(err),
        }
    })?;

    // Insert our newly parsed Cache-Control header.
    response
        .headers_mut()
        .insert("cache-control", cache_control_header);

    // if dl parameter is passed, set content-disposition header to attachment to force download
    if params.dl.is_some() {
        let content_disposition_header = format!("attachment; filename=\"{}\"", file.name)
            .parse()
            .map_err(|err| {
                error!("Invalid content disposition header: {}", file.name);
                Error::InternalServerError {
                    source: Box::new(err),
                }
            })?;
        response
            .headers_mut()
            .insert("content-disposition", content_disposition_header);
    }

    Ok(response)
}

#[derive(OpenApi)]
#[openapi(paths(static_files_handler))]
pub struct StaticFileApiDoc;
