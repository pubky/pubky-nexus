use crate::models::file::details::FileKey;
use crate::models::file::FileDetails;
use crate::routes::v0::endpoints::FILE_LIST_ROUTE;
use crate::{Error, Result};
use axum::extract::Query;
use axum::Json;
use log::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = FILE_LIST_ROUTE,
    tag = "Files by URIs",
    params(
        ("uris" = Vec<String>, Query, description = "List of Pubky Uris")
    ),
    responses(
        (status = 200, description = "List of File Details", body = Vec<FileDetails>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn file_details_by_uris_handler(
    Query(uris): Query<Vec<String>>,
) -> Result<Json<Vec<FileDetails>>> {
    info!("GET {FILE_LIST_ROUTE} uris:{:?}", uris);

    let file_uris: Vec<FileKey> = uris
        .iter()
        .map(|uri| FileDetails::file_key_from_uri(uri))
        .collect();

    match FileDetails::get_files(file_uris.iter().collect()).await {
        Ok(files) => Ok(Json(files)),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(file_details_by_uris_handler), components(schemas(FileDetails)))]
pub struct FilesListApiDoc;
