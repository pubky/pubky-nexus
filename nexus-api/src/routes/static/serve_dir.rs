use std::path::PathBuf;

use crate::{Error, Result};
use axum::{
    body::Body,
    http::{Request, StatusCode, Uri},
    response::Response,
};
use once_cell::sync::OnceCell;
use tower_http::services::{fs::ServeFileSystemResponseBody, ServeDir};
use tracing::error;

static SERVE_DIR_INSTANCE: OnceCell<ServeDir> = OnceCell::new();

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
        *request.uri_mut() =
            path.as_str()
                .parse::<Uri>()
                .map_err(|err| Error::InternalServerError {
                    source: Box::new(err),
                })?;

        let response_result = Self::get_serve_dir(file_path).try_call(request).await;

        let mut response = match response_result {
            Ok(response) => {
                if response.status() != StatusCode::OK {
                    return Ok(response);
                }
                response
            }
            Err(err) => {
                return Err(Error::InternalServerError {
                    source: Box::new(err),
                });
            }
        };

        // set the content type header
        let content_type_header = content_type.parse().map_err(|err| {
            error!("Invalid content type header: {}", content_type);
            Error::InternalServerError {
                source: Box::new(err),
            }
        })?;

        response
            .headers_mut()
            .insert("content-type", content_type_header);
        Ok(response)
    }
}
