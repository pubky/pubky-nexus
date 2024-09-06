use crate::models::file::FileDetails;
use crate::models::traits::Collection;
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

    let keys: Vec<Vec<String>> = uris
        .into_iter()
        .map(|uri| FileDetails::file_key_from_uri(uri.as_str()))
        .collect();

    let key_refs: Vec<Vec<&str>> = keys
        .iter()
        .map(|vec| vec.iter().map(|s| s.as_str()).collect())
        .collect();

    let slice_keys: Vec<&[&str]> = key_refs.iter().map(|arr| arr.as_slice()).collect();

    let files = FileDetails::get_by_ids(&slice_keys).await;

    match files {
        Ok(value) => {
            let data: Vec<FileDetails> = value.into_iter().filter_map(|val| val).collect();
            Ok(Json(data))
        }
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(file_details_by_uris_handler), components(schemas(FileDetails)))]
pub struct FilesListApiDoc;
