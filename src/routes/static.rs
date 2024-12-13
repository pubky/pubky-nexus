use std::borrow::BorrowMut;

use crate::{
    models::{
        file::{details::FileVersions, FileDetails},
        traits::Collection,
    },
    static_processor::{self, is_version_available},
    Config,
};
use axum::{
    extract::{Query, Request},
    middleware::{self, Next},
    response::Response,
    routing::get_service,
    Router,
};
use log::debug;
use reqwest::StatusCode;
use serde::Deserialize;
use tokio::fs;
use tower_http::services::ServeDir;

#[derive(Deserialize)]
pub struct FileParams {
    dl: Option<String>,
}

async fn static_files_middleware(
    params: Query<FileParams>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = String::from(request.uri().path());

    // path_parts: ["", "static", "files", "<USER_ID>", "<FILE_ID>", "<VERSION_NAME>"]
    let path_parts: Vec<&str> = path.split("/").collect();

    if path_parts.len() < 5 || path_parts.len() > 6 {
        return Err(StatusCode::NOT_FOUND);
    }

    let user_id = path_parts[3];
    let file_id = path_parts[4];
    if file_id.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }

    // for backward compatibility with old urls, replace old urls with main version of file
    let new_uri = if path_parts.len() == 5 {
        format!("{}/main", request.uri().path())
    } else if path_parts[5].is_empty() {
        format!("{}main", request.uri().path())
    } else {
        request.uri().path().to_string()
    };
    *request.borrow_mut().uri_mut() = new_uri.parse().unwrap();

    let version = if path_parts.len() == 6 {
        match path_parts[5].is_empty() {
            true => Some(FileVersions::MAIN),
            false => FileVersions::parse_from_str(path_parts[5]),
        }
    } else {
        Some(FileVersions::MAIN)
    };

    if version.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    let files =
        match FileDetails::get_by_ids(vec![vec![user_id, file_id].as_slice()].as_slice()).await {
            Ok(files) => files,
            Err(_) => {
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

    let file = match files[0].clone() {
        Some(file) => file,
        None => return Err(StatusCode::NOT_FOUND),
    };

    let version_value = version.unwrap();

    let version_available = is_version_available(&file.content_type, version_value.clone());
    if !version_available {
        return Err(StatusCode::BAD_REQUEST);
    }

    let file_version_content_type = ensure_file_version_exists(&file, version_value).await?;

    let mut response = next.run(request).await;

    if response.status() != StatusCode::OK {
        return Ok(response);
    }

    if params.dl.is_some() {
        response.headers_mut().insert(
            "content-disposition",
            format!("attachment; filename=\"{}\"", file.name)
                .parse()
                .unwrap(),
        );
    }

    response.headers_mut().insert(
        "content-type",
        file_version_content_type
            .unwrap_or(file.content_type)
            .parse()
            .unwrap(),
    );
    Ok(response)
}

pub async fn ensure_file_version_exists(
    file: &FileDetails,
    version: FileVersions,
) -> Result<Option<String>, StatusCode> {
    if version == FileVersions::MAIN {
        return Ok(None);
    }

    let path = format!("{}/{}/{}", file.owner_id, file.id, version);

    if fs::metadata(path).await.is_ok() {
        return Ok(None);
    }

    match static_processor::create_file_version(file, version).await {
        Ok(content_type) => Ok(Some(content_type)),
        Err(err) => {
            debug!("Failed to create file version: {:?}", err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub fn routes() -> Router {
    let config = Config::from_env();

    let general =
        Router::new().nest_service("/static/", get_service(ServeDir::new(config.static_path)));

    let files = Router::new()
        .nest_service(
            "/static/files/",
            get_service(ServeDir::new(config.file_path)),
        )
        .route_layer(middleware::from_fn(static_files_middleware));

    general.merge(files)
}
