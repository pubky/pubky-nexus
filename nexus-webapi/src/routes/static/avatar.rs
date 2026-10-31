use std::path::PathBuf;

use super::endpoints::USER_AVATAR_ROUTE;
use super::serve_dir::serve_file_variant;
use crate::models::PubkyId;
use crate::routes::AppState;
use crate::routes::Path;
use crate::{Error, Result};
use axum::extract::{Request, State};
use axum::http::{header, HeaderValue};
use axum::response::Response;
use nexus_common::media::FileVariant;
use nexus_common::models::{file::FileDetails, traits::Collection, user::UserDetails};
use tower_http::services::fs::ServeFileSystemResponseBody;
use tracing::{debug, warn};
use utoipa::OpenApi;

const AVATAR_FALLBACK_CACHE_CONTROL: &str = "public, max-age=30";

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
        (status = 400, description = "Invalid parameters"),
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

    // The profile `image` is free text from the user's homeserver. A URI that names no
    // pubky.app file record is a missing avatar, not a server fault.
    let Some((owner_id, file_id)) = FileDetails::file_key_from_uri(&image_uri) else {
        debug!("Avatar image is not a file URI for user: {user_id}");
        return Err(Error::FileNotFound {});
    };

    let file_list = FileDetails::get_by_ids(&[&[&owner_id, &file_id]]).await?;

    let Some(file_details) = file_list.into_iter().flatten().next() else {
        return Err(Error::FileNotFound {});
    };

    // No permit free: serve the untouched `main` image instead of queueing for one.
    // Queueing could still win a permit before the timeout, but that is not worth up to
    // 5s of latency on an avatar — and `small` is only deferred, not lost: the short
    // cache TTL brings the client back, and whichever request gets a permit writes the
    // variant to disk for every request after it.
    let controller = &app_state.fail_fast_variant_controller;
    let variant = match controller
        .ensure_variant(&file_details, &FileVariant::Small, file_path)
        .await
    {
        Ok(_) => FileVariant::Small,
        Err(ref e) if e.is_load_shed() => {
            warn!("Media processing unavailable ({e}) for user: {user_id} avatar with file: {file_id}, falling back to main");
            FileVariant::Main
        }
        Err(e) => return Err(e.into()),
    };

    let mut response = serve_file_variant(
        request,
        &file_details,
        &variant,
        file_path.clone(),
        false,
        controller,
    )
    .await?;

    // A degraded `main` fallback must not outlive the capacity blip that caused it,
    // so it gets a short TTL instead of the hour the real small variant earns.
    if variant != FileVariant::Small {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static(AVATAR_FALLBACK_CACHE_CONTROL),
        );
    }

    Ok(response)
}

#[derive(OpenApi)]
#[openapi(paths(user_avatar_handler), components(schemas(PubkyId)))]
pub struct UserAvatarApiDoc;
