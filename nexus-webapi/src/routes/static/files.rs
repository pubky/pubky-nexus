use std::path::PathBuf;

use axum::{
    extract::{Query, Request, State},
    response::Response,
};
use serde::Deserialize;
use tokio::fs;
use tower_http::services::fs::ServeFileSystemResponseBody;
use tracing::{debug, error};
use utoipa::OpenApi;

use super::endpoints::{STATIC_FILES_ROUTE, V1_STATIC_FILES_ROUTE};
use super::serve_dir::serve_file_variant;
use crate::media::MediaFile;
use crate::models::{FileId, FileRef, PubkyId};
use crate::routes::AppState;
use crate::routes::Path;
use crate::{Error, Result};
use nexus_common::{
    media::{content_type_for_ext, validate_variant_for_content_type, FileVariant},
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

/// The `/v1/static/files` path, whose `file_id` takes both id forms: it is read by
/// [`FileRef::parse`] once the owner is known, since a v1 id is checked as a whole address.
#[derive(Deserialize)]
pub struct V1FilePath {
    owner_id: PubkyId,
    file_id: String,
    variant: FileVariant,
}

/// Handler to serve static files
/// If the variant has not been created, it will be created on the fly
/// If the variant is not valid for the content type, a 400 Bad Request will be returned
/// If the file does not exist, a 404 Not Found will be returned
/// If the processing of the new variant fails, a 500 Internal Server Error will be returned
/// If the service is temporarily overloaded, a 503 Service Unavailable will be returned
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
        (status = 400, description = "Invalid parameters, or variant not valid for the file's content type"),
        (status = 404, description = "File not found"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error"),
        (status = 503, description = "Service temporarily unavailable; retry later")
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

    serve_legacy_file(
        request,
        &app_state,
        &owner_id,
        &file_id,
        &variant,
        params.dl.is_some(),
    )
    .await
}

/// Serves a file of either epoch under the one path a v1 client uses. A `{tsid}` is read from
/// its `File` node and legacy folder, as `/static/files` reads it, also after its owner migrated;
/// a `{hash}.{ext}` is read from the owner's v1 folder, with no node: its content type comes
/// from the extension and a download is named after the file. A `{hash}.{ext}` the spec
/// crate's reader does not accept, such as `H.JPG`, `H.tar.gz` or a bare `H`, is a 400. A v1
/// file whose `main` is not on disk is a 404 before any variant is derived.
#[utoipa::path(
    get,
    path = V1_STATIC_FILES_ROUTE,
    description = "Serves a file of either epoch by owner_id, file_id and variant. The file id is a 13-character id of a v0 file, or the `{hash}.{ext}` name of a social/v1 file",
    tag = "File",
    params(
        ("owner_id" = PubkyId, Path, description = "File's owner id"),
        ("file_id" = String, Path, description = "File's id: a 13-character Crockford Base32 id, or a `{hash}.{ext}` social/v1 file name"),
        ("variant" = FileVariant, Path, description = "File's variant"),
        ("dl" = Option<String>, Query, description = "Download the file. Returns with content-disposition header set to attachment")
    ),
    responses(
        (status = 200, description = "File's raw data"),
        (status = 400, description = "Invalid parameters, a file id of neither form, or variant not valid for the file's content type"),
        (status = 404, description = "File not found"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error"),
        (status = 503, description = "Service temporarily unavailable; retry later")
    )
)]
pub async fn v1_static_files_handler(
    Path(V1FilePath {
        owner_id,
        file_id,
        variant,
    }): Path<V1FilePath>,
    State(app_state): State<AppState>,
    params: Query<FileParams>,
    request: Request,
) -> Result<Response<ServeFileSystemResponseBody>> {
    debug!(
        "Serving v1 route file for user: {} and file: {} with variant: {:?}",
        owner_id, file_id, variant
    );
    let download = params.dl.is_some();

    match FileRef::parse(&owner_id, file_id)? {
        FileRef::Legacy(file_id) => {
            serve_legacy_file(request, &app_state, &owner_id, &file_id, &variant, download).await
        }
        FileRef::V1 { filename, ext } => {
            let file = MediaFile {
                owner_id: owner_id.to_string(),
                id: filename.clone(),
                content_type: content_type_for_ext(&ext).to_string(),
                download_name: Some(filename),
            };
            serve_v1_file(request, &app_state, file, &variant, download).await
        }
    }
}

/// Serves a v0 file from its `File` node and legacy folder: 404 without the node, 400 for a
/// variant its content type lacks. Shared by `/static/files` and the `{tsid}` form of
/// `/v1/static/files`, so the two cannot drift.
async fn serve_legacy_file(
    request: Request,
    app_state: &AppState,
    owner_id: &PubkyId,
    file_id: &FileId,
    variant: &FileVariant,
    download: bool,
) -> Result<Response<ServeFileSystemResponseBody>> {
    let file_path: &PathBuf = &app_state.files_path;

    let files = FileDetails::get_by_ids(vec![vec![&**owner_id, &**file_id].as_slice()].as_slice())
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

    if !validate_variant_for_content_type(file.content_type.as_str(), variant) {
        return Err(Error::invalid_input(format!(
            "variant {} is not valid for content type {}",
            variant, file.content_type
        )));
    }

    serve_file_variant(
        request,
        &MediaFile::from(&file),
        variant,
        file_path.clone(),
        download,
        &app_state.queued_variant_controller,
    )
    .await
}

/// Serves a `social/v1` file from the owner's v1 folder. A v1 file has no node, so what stands
/// in for the node lookup is its `main` on disk: missing, the file is a 404, checked before
/// anything else because the variant controller reports `main` as present without looking,
/// and a request for a file that is not there must start no conversion. Then the variant is
/// checked against the type, a 400 as for a v0 file.
async fn serve_v1_file(
    request: Request,
    app_state: &AppState,
    file: MediaFile,
    variant: &FileVariant,
    download: bool,
) -> Result<Response<ServeFileSystemResponseBody>> {
    let files_path: &PathBuf = &app_state.files_path;

    let main = files_path
        .join(file.folder())
        .join(FileVariant::Main.to_string());
    if !fs::try_exists(main).await? {
        return Err(Error::FileNotFound {});
    }

    if !validate_variant_for_content_type(&file.content_type, variant) {
        return Err(Error::invalid_input(format!(
            "variant {} is not valid for content type {}",
            variant, file.content_type
        )));
    }

    serve_file_variant(
        request,
        &file,
        variant,
        files_path.clone(),
        download,
        &app_state.queued_variant_controller,
    )
    .await
}

#[derive(OpenApi)]
#[openapi(
    paths(static_files_handler, v1_static_files_handler),
    components(schemas(FileVariant, PubkyId, FileId))
)]
pub struct StaticFileApiDoc;
