use std::path::PathBuf;
use std::sync::OnceLock;

use crate::Result;
use axum::{
    body::Body,
    http::{header, HeaderValue, Request, Uri},
    response::Response,
};
use nexus_common::{
    media::FileVariant,
    models::file::{Blob, FileDetails},
};
use tower_http::services::{fs::ServeFileSystemResponseBody, ServeDir};
use tracing::error;

static SERVE_DIR_INSTANCE: OnceLock<ServeDir> = OnceLock::new();

const CACHE_CONTROL: &str = "public, max-age=3600";

pub struct PubkyServeDir;

/// Wrapper around ServeDir to serve files from the configured directory
impl PubkyServeDir {
    fn get_serve_dir(file_path: PathBuf) -> ServeDir {
        SERVE_DIR_INSTANCE
            .get_or_init(|| ServeDir::new(file_path))
            .to_owned()
    }

    pub async fn try_call(
        mut request: Request<Body>,
        path: String,
        content_type: String,
        file_path: PathBuf,
    ) -> Result<Response<ServeFileSystemResponseBody>> {
        *request.uri_mut() = path.as_str().parse::<Uri>()?;

        let response_result = Self::get_serve_dir(file_path).try_call(request).await;

        let mut response = match response_result {
            // In case of success, extract the response for content-type header injection
            // is_success() checks for all success codes: 200 OK, but also 206 Partial Content
            // 206 is used for video streaming
            Ok(response) if response.status().is_success() => response,

            // In all other cases, return right away and skip content-type header injection
            Ok(response) => return Ok(response),
            Err(err) => return Err(err.into()),
        };

        // set the content type header
        let content_type_header = content_type
            .parse()
            .inspect_err(|_| error!("Invalid content type header: {}", content_type))?;

        response
            .headers_mut()
            .insert("content-type", content_type_header);
        Ok(response)
    }
}

/// Ensures a file variant exists, serves it, and applies cache headers.
/// If `download` is true, sets `Content-Disposition: attachment`.
pub async fn serve_file_variant(
    request: Request<Body>,
    file: &FileDetails,
    variant: &FileVariant,
    files_path: PathBuf,
    download: bool,
) -> Result<Response<ServeFileSystemResponseBody>> {
    let content_type = Blob::get_by_id(file, variant, files_path.clone()).await?;

    let disk_path = format!("/{}/{}/{variant}", file.owner_id, file.id);

    let mut response =
        PubkyServeDir::try_call(request, disk_path, content_type, files_path).await?;

    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(CACHE_CONTROL),
    );

    if download {
        let filename = &file.name;
        let content_disposition = format!("attachment; filename=\"{filename}\"")
            .parse()
            .inspect_err(|_| error!("Invalid content disposition header: {filename}"))?;
        headers.insert(header::CONTENT_DISPOSITION, content_disposition);
    }

    Ok(response)
}
