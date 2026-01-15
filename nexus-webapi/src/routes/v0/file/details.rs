use crate::routes::v0::endpoints::FILE_ROUTE;
use crate::{Error, Result};
use axum::extract::Path;
use axum::Json;
use nexus_common::models::file::FileDetails;
use nexus_common::models::file::FileUrls;
use nexus_common::models::traits::Collection;
use tracing::debug;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = FILE_ROUTE,
    description = "File details",
    tag = "File",
    params(
        ("file_id" = String, Path, description = "File Pubky Uri")
    ),
    responses(
        (status = 200, description = "File Details", body = FileDetails),
        (status = 404, description = "File not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn file_details_handler(Path(file_uri): Path<String>) -> Result<Json<FileDetails>> {
    debug!("GET {FILE_ROUTE} file_uri:{}", file_uri);

    let file_key = FileDetails::file_key_from_uri(&file_uri);
    let files = FileDetails::get_by_ids(&[&[&file_key[0], &file_key[1]]]).await?;

    let file = &files[0];
    match file {
        None => Err(Error::FileNotFound {}),
        Some(value) => Ok(Json(value.clone())),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(file_details_handler),
    components(schemas(FileDetails, FileUrls))
)]
pub struct FileDetailsApiDoc;
