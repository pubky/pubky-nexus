use axum::{
    extract::{Path, Query, Request},
    http::{StatusCode, Uri},
    response::Response,
};
use log::{debug, error};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use tower_http::services::{fs::ServeFileSystemResponseBody, ServeDir};

use crate::{
    models::{
        file::{details::FileVariant, FileDetails},
        traits::Collection,
    },
    static_processor::StaticProcessor,
    Config, Error, Result,
};

#[derive(Deserialize, Serialize)]
pub struct FileParams {
    // dl is download parameter to set the content-disposition header to attachment
    dl: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct FilePath {
    owner_id: String,
    file_id: String,
    variant: FileVariant,
}

/// Handler to serve static files
/// If the variant has not been created, it will be created on the fly
/// If the variant is not valid for the content type, a 400 Bad Request will be returned
/// If the file does not exist, a 404 Not Found will be returned
/// If the processing of the new variant fails, a 500 Internal Server Error will be returned
pub async fn static_files_handler(
    Path(FilePath {
        owner_id,
        file_id,
        variant,
    }): Path<FilePath>,
    params: Query<FileParams>,
    request: Request,
) -> Result<Response<ServeFileSystemResponseBody>> {
    debug!(
        "Serving file for user: {} and file: {} with variant: {:?}",
        owner_id, file_id, variant
    );
    let files = FileDetails::get_by_ids(
        vec![vec![owner_id.as_str(), file_id.as_str()].as_slice()].as_slice(),
    )
    .await
    .map_err(|err| {
        error!(
            "Error while fetching file details for user: {} and file: {}",
            owner_id, file_id
        );
        Error::InternalServerError { source: err }
    })?;

    if files.is_empty() {
        return Err(Error::FileNotFound {});
    }

    let file = files
        .first()
        .and_then(Clone::clone)
        .ok_or(Error::FileNotFound {})?;

    if !StaticProcessor::validate_variant_for_content_type(file.content_type.as_str(), &variant) {
        return Err(Error::InvalidInput {
            message: format!(
                "variant {} is not valid for content type {}",
                variant, file.content_type
            ),
        });
    }

    let file_variant_exists = StaticProcessor::check_variant_exists(&file, variant.clone()).await;

    let file_variant_content_type = if file_variant_exists {
        StaticProcessor::get_content_type_for_variant(&file, &variant)
    } else {
        match StaticProcessor::create_file_variant(&file, variant).await {
            Ok(content_type) => content_type,
            Err(err) => {
                error!(
                    "Creating variant failed for file: {:?} with error: {}",
                    file, err
                );
                return Err(Error::InternalServerError { source: err });
            }
        }
    };

    // Create a new request with a modified path to serve the file using ServeDir
    let (request_parts, request_body) = request.into_parts();
    let mut req = Request::from_parts(request_parts.clone(), request_body);
    *req.uri_mut() = request_parts
        .uri
        .path()
        .replace("static/files", "")
        .as_str()
        .parse::<Uri>()
        .map_err(|err| Error::InternalServerError {
            source: Box::new(err),
        })?;
    let response_result = get_serve_dir().try_call(req).await;

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
    let content_type_header = file_variant_content_type.parse().map_err(|err| {
        error!("Invalid content type header: {}", file_variant_content_type);
        Error::InternalServerError {
            source: Box::new(err),
        }
    })?;
    response
        .headers_mut()
        .insert("content-type", content_type_header);

    // if dl parameter is passed, set content-disposition header to attachment to force download
    if params.dl.is_some() {
        let content_disposition_header = format!("attachment; filename=\"{}\"", file.name)
            .parse()
            .map_err(|err| {
                error!("Invalid content disposition header: {}", file.name);
                Error::InternalServerError {
                    source: Box::new(err),
                }
            })?;
        response
            .headers_mut()
            .insert("content-disposition", content_disposition_header);
    }

    Ok(response)
}

static SERVE_DIR_INSTANCE: OnceCell<ServeDir> = OnceCell::new();

fn get_serve_dir() -> ServeDir {
    SERVE_DIR_INSTANCE
        .get_or_init(|| {
            let config = Config::from_env();
            ServeDir::new(config.file_path)
        })
        .to_owned()
}
