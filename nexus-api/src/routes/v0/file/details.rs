use crate::routes::v0::endpoints::FILE_ROUTE;
use crate::{Error, Result};
use axum::extract::Path;
use axum::Json;
use nexus_common::models::file::FileDetails;
use nexus_common::models::file::FileUrls;
use nexus_common::models::traits::Collection;
use tracing::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = FILE_ROUTE,
    description = "File details",
    tag = "File",
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
    let result = FileDetails::get_by_ids(
        vec![vec![file_key[0].as_str(), file_key[1].as_str()].as_slice()].as_slice(),
    )
    .await;

    match result {
        Ok(files) => {
            let file = &files[0];
            match file {
                None => Err(Error::FileNotFound {}),
                Some(value) => Ok(Json(value.clone())),
            }
        }
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(file_details_handler),
    components(schemas(FileDetails, FileUrls))
)]
pub struct FileDetailsApiDoc;
