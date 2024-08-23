use crate::models::file::FileDetails;
use crate::routes::v0::endpoints::FILE_ROUTE;
use crate::{Error, Result};
use axum::extract::Path;
use axum::Json;
use log::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = FILE_ROUTE,
    tag = "File Details",
    params(
        ("file_uri" = String, Path, description = "File Pubky Uri")
    ),
    responses(
        (status = 200, description = "File Details", body = FileDetails),
        (status = 404, description = "File not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn file_details_handler(Path(file_uri): Path<String>) -> Result<Json<FileDetails>> {
    info!("GET {FILE_ROUTE} file_uri:{}", file_uri);

    let file_key = FileDetails::file_key_from_uri(&file_uri);

    match FileDetails::get_file(&file_key).await {
        Ok(Some(file)) => Ok(Json(file)),
        Ok(None) => Err(Error::FileNotFound {}),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(file_details_handler), components(schemas(FileDetails)))]
pub struct FileDetailsApiDoc;
