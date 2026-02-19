use crate::routes::v0::endpoints::FILE_LIST_ROUTE;
use crate::Result;
use axum::Json;
use nexus_common::models::file::FileDetails;
use nexus_common::models::traits::Collection;
use serde::Deserialize;
use tracing::{debug, warn};
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
    debug!("GET {FILE_LIST_ROUTE} uris:{:?}", body.uris);

    let keys: Vec<(String, String)> = body
        .uris
        .iter()
        .filter_map(|uri| {
            let key = FileDetails::file_key_from_uri(uri.as_str());
            if key.is_none() {
                warn!("Skipping invalid file URI: {}", uri);
            }
            key
        })
        .collect();

    let key_refs: Vec<[&str; 2]> = keys
        .iter()
        .map(|(owner, id)| [owner.as_str(), id.as_str()])
        .collect();

    let slice_keys: Vec<&[&str]> = key_refs.iter().map(|arr| arr.as_slice()).collect();

    let files = FileDetails::get_by_ids(&slice_keys).await?;
    let data: Vec<FileDetails> = files.into_iter().flatten().collect();
    Ok(Json(data))
}

#[derive(OpenApi)]
#[openapi(
    paths(file_details_by_uris_handler),
    components(schemas(FileDetails, FilesByIdsBody))
)]
pub struct FilesListApiDoc;
