use crate::models::file::details::FileVariant;
use crate::routes::r#static::PubkyServeDir;
use crate::static_processor::StaticProcessor;
use crate::{
    models::{file::FileDetails, traits::Collection, user::UserDetails},
    Error, Result,
};
use axum::extract::Request;
use axum::{extract::Path, response::Response};
use tower_http::services::fs::ServeFileSystemResponseBody;
use tracing::{error, info};
use utoipa::OpenApi;

use super::endpoints::USER_AVATAR_ROUTE;

#[utoipa::path(
    get,
    path = USER_AVATAR_ROUTE,
    description = "Get the user's avatar image",
    tag = "User",
    params(
        ("user_id" = String, Path, description = "Pubky user ID whose avatar we want")
    ),
    responses(
        (status = 200, description = "Avatar image"),
        (status = 404, description = "User or avatar not found"),
        (status = 500, description = "Internal error retrieving avatar")
    )
)]
pub async fn user_avatar_handler(
    Path(user_id): Path<String>,
    request: Request,
) -> Result<Response<ServeFileSystemResponseBody>> {
    info!("GET {USER_AVATAR_ROUTE} user_id:{}", user_id);

    // 1. Get user details
    let details = match UserDetails::get_by_id(&user_id)
        .await
        .map_err(|source| Error::InternalServerError { source })?
    {
        None => return Err(Error::UserNotFound { user_id }),
        Some(d) => d,
    };

    // 2. Check if user has image. If not, 404
    let Some(image_uri) = details.image else {
        return Err(Error::FileNotFound {});
    };

    // 3. Parse user_id + file_id from the "pubky://owner_id/file_id" style URI
    let keys = FileDetails::file_key_from_uri(&image_uri);
    if keys.len() != 2 {
        return Err(Error::InternalServerError {
            source: format!("Invalid file URI: {image_uri}").into(),
        });
    }
    let (owner_id, file_id) = (keys[0].clone(), keys[1].clone());

    // 4. Look up FileDetails in Redis/Neo4j using get_by_ids
    let file_list = FileDetails::get_by_ids(&[&[&owner_id, &file_id]])
        .await
        .map_err(|source| Error::InternalServerError { source })?;

    // We expect only one result in file_list, a Vec<Option<FileDetails>>
    let Some(file_details) = file_list.into_iter().flatten().next() else {
        return Err(Error::FileNotFound {});
    };

    // 5. ensure small variant is created
    let small_variant_content_type =
        StaticProcessor::get_or_create_variant(&file_details, &FileVariant::Small)
            .await
            .map_err(|err| {
                error!(
                    "Error while processing small variant for user: {} avatar with file: {}",
                    user_id, file_id
                );
                Error::InternalServerError { source: err }
            })?;

    // serve the file using ServeDir
    // Create a new request with a modified path to serve the file using ServeDir
    // 6. Build the url using small variant
    let file_uri_path = format!(
        "/{}/{}/{}", // /{owner_id}/{file_id}/{variant}
        user_id,
        file_details.id,
        FileVariant::Small,
    );

    PubkyServeDir::try_call(request, file_uri_path, small_variant_content_type).await
}

#[derive(OpenApi)]
#[openapi(paths(user_avatar_handler))]
pub struct UserAvatarApiDoc;
