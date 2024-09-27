use crate::{
    models::{
        file::{details::FileVersions, FileDetails},
        traits::Collection,
    },
    static_processor::{self},
    Config,
};
use axum::{
    extract::{Query, Request},
    middleware::{self, Next},
    response::Response,
    routing::get_service,
    Error, Router,
};
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
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = String::from(request.uri().path());

    // path_parts: ["", "static", "files", "<USER_ID>", "<FILE_ID>", "<VERSION_NAME>"]
    let path_parts: Vec<&str> = path.split("/").collect();

    if path_parts.len() < 5 {
        return Err(StatusCode::NOT_FOUND);
    }

    let user_id = path_parts[3];
    let file_id = path_parts[4];
    let version_name = match path_parts.len() {
        6 => path_parts[5],
        _ => &FileVersions::MAIN.to_string(),
    };
    let version = FileVersions::from_str(version_name);

    if version.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    let files = FileDetails::get_by_ids(vec![vec![user_id, file_id].as_slice()].as_slice()).await;

    if files.is_err() {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let files_values = files.unwrap();
    let file = files_values[0].clone();

    if file.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    let file_value = file.unwrap();

    let file_version = ensure_file_version_exists(&file_value, version.unwrap()).await;

    if file_version.is_err() {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let mut response = next.run(request).await;

    if response.status() != StatusCode::OK {
        return Ok(response);
    }

    if params.dl.is_some() {
        response.headers_mut().insert(
            "content-disposition",
            format!("attachment; filename=\"{}\"", file_value.name)
                .parse()
                .unwrap(),
        );
    }

    response
        .headers_mut()
        .insert("content-type", file_value.content_type.parse().unwrap());
    Ok(response)
}

pub async fn ensure_file_version_exists(
    file: &FileDetails,
    version: FileVersions,
) -> Result<(), Error> {
    if version == FileVersions::MAIN {
        return Ok(());
    }

    let path = format!("{}/{}/{}", file.owner_id, file.id, version);

    if fs::metadata(path).await.is_ok() {
        return Ok(());
    }

    static_processor::create_file_version(file, version).await
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
