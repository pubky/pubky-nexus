use crate::routes::v0::endpoints::FILE_ROUTE;
use crate::routes::Path;
use crate::{Error, Result};
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
        (status = 400, description = "Malformed file URI"),
        (status = 404, description = "File not found"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn file_details_handler(Path(file_uri): Path<String>) -> Result<Json<FileDetails>> {
    debug!("GET {FILE_ROUTE} file_uri:{}", file_uri);

    let (owner_id, file_id) = FileDetails::file_key_from_uri(&file_uri)
        .ok_or(Error::invalid_input("Malformed file URI"))?;
    let files = FileDetails::get_by_ids(&[&[&owner_id, &file_id]]).await?;

    match &files[0] {
        None => Err(Error::FileNotFound {}),
        Some(value) => {
            // `urls` is written at ingestion, so a file indexed before a variant existed has no
            // entry for it; the static route would still serve that variant. Fill the derived
            // ones here so a reader never has to know when the file was indexed.
            let mut file = value.clone();
            file.urls.fill_derived_variants(&file.content_type);
            Ok(Json(file))
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(file_details_handler),
    components(schemas(FileDetails, FileUrls))
)]
pub struct FileDetailsApiDoc;
