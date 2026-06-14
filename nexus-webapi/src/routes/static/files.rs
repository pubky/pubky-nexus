use std::path::PathBuf;

use axum::{
    extract::{Query, Request, State},
    response::Response,
};
use serde::Deserialize;
use tower_http::services::fs::ServeFileSystemResponseBody;
use tracing::{debug, error};
use utoipa::OpenApi;

use super::endpoints::STATIC_FILES_ROUTE;
use super::serve_dir::serve_file_variant;
use crate::models::{FileId, PubkyId};
use crate::routes::AppState;
use crate::routes::Path;
use crate::{Error, Result};
use nexus_common::{
    media::{FileVariant, VariantController},
    models::{file::FileDetails, traits::Collection},
};

#[derive(Deserialize)]
pub struct FileParams {
    // dl is download parameter to set the content-disposition header to attachment
    dl: Option<String>,
}

#[derive(Deserialize)]
pub struct FilePath {
    owner_id: PubkyId,
    file_id: FileId,
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
        ("owner_id" = PubkyId, Path, description = "File's owner id"),
        ("file_id" = FileId, Path, description = "File's id"),
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

    let files = FileDetails::get_by_ids(vec![vec![&*owner_id, &*file_id].as_slice()].as_slice())
        .await
        .inspect_err(|_| {
            error!("Error while fetching file details for user: {owner_id} and file: {file_id}")
        })?;

    if files.is_empty() {
        return Err(Error::FileNotFound {});
    }

    let file = files
        .first()
        .and_then(Clone::clone)
        .ok_or(Error::FileNotFound {})?;

    if !VariantController::validate_variant_for_content_type(file.content_type.as_str(), &variant) {
        return Err(Error::invalid_input(&format!(
            "variant {} is not valid for content type {}",
            variant, file.content_type
        )));
    }

    serve_file_variant(
        request,
        &file,
        &variant,
        file_path.clone(),
        params.dl.is_some(),
    )
    .await
    .inspect_err(|_| {
        error!("Error while processing file variant for variant: {variant} and file: {file_id}")
    })
}

#[derive(OpenApi)]
#[openapi(
    paths(static_files_handler),
    components(schemas(FileVariant, PubkyId, FileId))
)]
pub struct StaticFileApiDoc;
