use crate::routes::v0::endpoints::USER_AVATAR_ROUTE;
use crate::{
    models::{file::FileDetails, traits::Collection, user::UserDetails},
    Config, Error, Result,
};
use axum::{
    extract::Path,
    http::{header::CONTENT_TYPE, HeaderValue},
    response::Response,
};
use log::{info, warn};
use utoipa::OpenApi;

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
pub async fn user_avatar_handler(Path(user_id): Path<String>) -> Result<Response> {
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

    // 5. Build the actual path to the file on disk
    let config = Config::from_env();
    let file_path = format!("{}/{}/{}", config.file_path, user_id, file_details.id);

    // 6. Read the file bytes from disk
    let data = match tokio::fs::read(&file_path).await {
        Ok(buf) => buf,
        Err(err) => {
            warn!("Reading avatar file failed: {err}");
            return Err(Error::FileNotFound {});
        }
    };

    let content_type = file_details.content_type.clone();

    // 7. Build Axum `Response` setting the correct Content-Type
    let mut response = Response::new(data.into());
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_str(&content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );

    Ok(response)
}

#[derive(OpenApi)]
#[openapi(paths(user_avatar_handler))]
pub struct UserAvatarApiDoc;
