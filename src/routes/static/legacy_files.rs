use crate::{models::file::details::FileVariant, Result};
use axum::{
    extract::Request,
    response::{IntoResponse, Redirect, Response},
};

/// Handler to redirect legacy static files
/// The path should be in the format /static/{owner_id}/{file_id}
pub async fn legacy_files_handler(request: Request) -> Result<Response> {
    // Construct the new path
    let new_path = format!("{}/{}", request.uri().path(), FileVariant::Main);

    // Perform a redirect to the new path
    Ok(Redirect::permanent(&new_path).into_response())
}
