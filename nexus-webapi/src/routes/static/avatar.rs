use std::path::PathBuf;

use super::endpoints::USER_AVATAR_ROUTE;
use super::serve_dir::serve_file_variant;
use crate::models::PubkyId;
use crate::routes::AppState;
use crate::routes::Path;
use crate::{Error, Result};
use axum::extract::{Request, State};
use axum::response::Response;
use nexus_common::media::FileVariant;
use nexus_common::models::{file::FileDetails, traits::Collection, user::UserDetails};
use tower_http::services::fs::ServeFileSystemResponseBody;
use tracing::{debug, error};
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = USER_AVATAR_ROUTE,
    description = "Get the user's avatar image",
    tag = "User",
    params(
        ("user_id" = PubkyId, Path, description = "Pubky user ID whose avatar we want")
    ),
    responses(
        (status = 200, description = "Avatar image"),
        (status = 404, description = "User or avatar not found"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal error retrieving avatar")
    )
)]
pub async fn user_avatar_handler(
    Path(user_id): Path<PubkyId>,
    State(app_state): State<AppState>,
    request: Request,
) -> Result<Response<ServeFileSystemResponseBody>> {
    debug!("GET {USER_AVATAR_ROUTE} user_id:{}", user_id);

    let file_path: &PathBuf = &app_state.files_path;

    let details = match UserDetails::get_by_id(&user_id).await? {
        None => return Err(Error::user_not_found(user_id)),
        Some(d) => d,
    };

    let Some(image_uri) = details.image else {
        return Err(Error::FileNotFound {});
    };

    let (owner_id, file_id) =
        FileDetails::file_key_from_uri(&image_uri).ok_or(Error::InternalServerError {
            source: format!("Invalid file URI: {image_uri}").into(),
        })?;

    let file_list = FileDetails::get_by_ids(&[&[&owner_id, &file_id]]).await?;

    let Some(file_details) = file_list.into_iter().flatten().next() else {
        return Err(Error::FileNotFound {});
    };

    serve_file_variant(
        request,
        &file_details,
        &FileVariant::Small,
        file_path.clone(),
        false,
    )
    .await
    .inspect_err(|_| {
        error!(
            "Error while processing small variant for user: {user_id} avatar with file: {file_id}"
        )
    })
}

#[derive(OpenApi)]
#[openapi(paths(user_avatar_handler), components(schemas(PubkyId)))]
pub struct UserAvatarApiDoc;
