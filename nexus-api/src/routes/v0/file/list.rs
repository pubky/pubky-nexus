use crate::routes::v0::endpoints::FILE_LIST_ROUTE;
use crate::{Error, Result};
use axum::Json;
use nexus_common::models::file::FileDetails;
use nexus_common::models::traits::Collection;
use serde::Deserialize;
use tracing::info;
use utoipa::{OpenApi, ToSchema};

#[derive(Deserialize, ToSchema)]
pub struct FilesByIdsBody {
    uris: Vec<String>,
}

#[utoipa::path(
    post,
    path = FILE_LIST_ROUTE,
    description = "List files by URIs. This is a POST request because we're passing a potentially large list of file URIs in the request body.",
    tag = "File",
    request_body = FilesByIdsBody,
    responses(
        (status = 200, description = "List of File Details", body = Vec<FileDetails>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn file_details_by_uris_handler(
    Json(body): Json<FilesByIdsBody>,
) -> Result<Json<Vec<FileDetails>>> {
    info!("GET {FILE_LIST_ROUTE} uris:{:?}", body.uris);

    let keys: Vec<Vec<String>> = body
        .uris
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
            let data: Vec<FileDetails> = value.into_iter().flatten().collect();
            Ok(Json(data))
        }
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(file_details_by_uris_handler),
    components(schemas(FileDetails, FilesByIdsBody))
)]
pub struct FilesListApiDoc;
