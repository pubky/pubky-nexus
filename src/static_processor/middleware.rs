use axum::{
    extract::{Path, Query, Request},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use log::error;
use serde::{Deserialize, Serialize};

use crate::models::{
    file::{details::FileVariant, FileDetails},
    traits::Collection,
};

use super::StaticProcessor;

#[derive(Deserialize, Serialize)]
pub struct FileParams {
    dl: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct FilePath {
    owner_id: String,
    file_id: String,
    variant: FileVariant,
}

/// Middleware to serve static files
/// The path should be in the format /static/{owner_id}/{file_id}/{variant}
/// If the variant has not been created, it will be created on the fly
/// If the variant is not valid for the content type, a 400 Bad Request will be returned
/// If the file does not exist, a 404 Not Found will be returned
/// If the processing of the new variant fails, a 500 Internal Server Error will be returned
pub async fn static_files_middleware(
    Path(FilePath {
        owner_id,
        file_id,
        variant,
    }): Path<FilePath>,
    params: Query<FileParams>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let files = match FileDetails::get_by_ids(
        vec![vec![owner_id.as_str(), file_id.as_str()].as_slice()].as_slice(),
    )
    .await
    {
        Ok(files) => files,
        Err(_) => {
            error!(
                "Error while fetching file details for user: {} and file: {}",
                owner_id, file_id
            );
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    if files.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }

    let file = match files[0].clone() {
        Some(file) => file,
        None => return Err(StatusCode::NOT_FOUND),
    };

    if !StaticProcessor::validate_variant_for_content_type(
        file.content_type.as_str(),
        variant.clone(),
    ) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let file_variant_exists =
        StaticProcessor::check_variant_existence(&file, variant.clone()).await;

    let file_variant_content_type = if file_variant_exists {
        file.content_type.clone()
    } else {
        match StaticProcessor::create_file_variant(&file, variant).await {
            Ok(content_type) => content_type,
            Err(err) => {
                error!(
                    "Creating variant failed for file: {:?} with error: {}",
                    file, err
                );
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    };

    let mut response = next.run(request).await;

    // if serving the file was not successful, return the response as is
    if response.status() != StatusCode::OK {
        return Ok(response);
    }

    // set the content type header
    let content_type_header = match file_variant_content_type.parse() {
        Ok(content_type) => content_type,
        Err(_) => {
            error!("Invalid content type header: {}", file_variant_content_type);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    response
        .headers_mut()
        .insert("content-type", content_type_header);

    // if dl parameter is passed, set content-disposition header to attachment to force download
    if params.dl.is_some() {
        let content_disposition_header =
            match format!("attachment; filename=\"{}\"", file.name).parse() {
                Ok(content_disposition) => content_disposition,
                Err(_) => {
                    error!("Invalid content disposition header: {}", file.name);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            };
        response
            .headers_mut()
            .insert("content-disposition", content_disposition_header);
    }

    Ok(response)
}
