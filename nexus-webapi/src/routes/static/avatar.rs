use std::path::{Path as FsPath, PathBuf};

use super::endpoints::{USER_AVATAR_ROUTE, V1_USER_AVATAR_ROUTE};
use super::files::v1_file_is_on_disk;
use super::serve_dir::serve_file_variant;
use crate::media::MediaFile;
use crate::models::{FileRef, PubkyId};
use crate::routes::AppState;
use crate::routes::Path;
use crate::{Error, Result};
use axum::extract::{Request, State};
use axum::http::{header, HeaderValue};
use axum::response::Response;
use nexus_common::media::{content_type_for_ext, FileVariant, MediaKind};
use nexus_common::models::{file::FileDetails, traits::Collection, user::UserDetails};
use tower_http::services::fs::ServeFileSystemResponseBody;
use tracing::{debug, warn};
use utoipa::OpenApi;

const AVATAR_FALLBACK_CACHE_CONTROL: &str = "public, max-age=30";

#[utoipa::path(
    get,
    path = USER_AVATAR_ROUTE,
    description = "Get the user's avatar image: the small variant of the profile image, when it is an image file Nexus holds",
    tag = "User",
    params(
        ("user_id" = PubkyId, Path, description = "Pubky user ID whose avatar we want")
    ),
    responses(
        (status = 200, description = "Avatar image"),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "User not found, no profile image, or an image Nexus does not hold: a web URL, another app's object, or a social/v1 file that is not an image or not on disk"),
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
    serve_avatar(user_id, app_state, request).await
}

/// The same handler under the v1 base path, so a v1 client reads an avatar where it reads every
/// other file's bytes and never picks a route from an address.
#[utoipa::path(
    get,
    path = V1_USER_AVATAR_ROUTE,
    description = "Get the user's avatar image: the same handler as `/static/avatar/{user_id}`, under the v1 base path",
    tag = "User",
    params(
        ("user_id" = PubkyId, Path, description = "Pubky user ID whose avatar we want")
    ),
    responses(
        (status = 200, description = "Avatar image"),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "User not found, no profile image, or an image Nexus does not hold: a web URL, another app's object, or a social/v1 file that is not an image or not on disk"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal error retrieving avatar")
    )
)]
pub async fn v1_user_avatar_handler(
    Path(user_id): Path<PubkyId>,
    State(app_state): State<AppState>,
    request: Request,
) -> Result<Response<ServeFileSystemResponseBody>> {
    debug!("GET {V1_USER_AVATAR_ROUTE} user_id:{}", user_id);
    serve_avatar(user_id, app_state, request).await
}

/// Serves the `small` variant of the user's profile image when it is an image file Nexus holds,
/// else a 404 a client can fall back from. Anything else a profile may name, a web URL or another
/// app's object, is neither redirected to, since the viewer's browser would then contact any
/// server a profile names, nor fetched, which would be server-side request forgery: a client that
/// wants it shows it itself from `image` in the user details.
async fn serve_avatar(
    user_id: PubkyId,
    app_state: AppState,
    request: Request,
) -> Result<Response<ServeFileSystemResponseBody>> {
    let file_path: &PathBuf = &app_state.files_path;

    let details = match UserDetails::get_by_id(&user_id).await? {
        None => return Err(Error::user_not_found(user_id)),
        Some(d) => d,
    };

    let Some(image_uri) = details.image else {
        return Err(Error::FileNotFound {});
    };

    let Some(file) = avatar_file(&image_uri, file_path).await? else {
        return Err(Error::FileNotFound {});
    };

    // No permit free: serve the untouched `main` image instead of queueing for one.
    // Queueing could still win a permit before the timeout, but that is not worth up to
    // 5s of latency on an avatar — and `small` is only deferred, not lost: the short
    // cache TTL brings the client back, and whichever request gets a permit writes the
    // variant to disk for every request after it.
    let controller = &app_state.fail_fast_variant_controller;
    let variant = match controller
        .ensure_variant(&file, &FileVariant::Small, file_path)
        .await
    {
        Ok(_) => FileVariant::Small,
        Err(ref e) if e.is_load_shed() => {
            warn!(
                "Media processing unavailable ({e}) for user: {user_id} avatar with file: {}, falling back to main",
                file.id
            );
            FileVariant::Main
        }
        Err(e) => return Err(e.into()),
    };

    let mut response = serve_file_variant(
        request,
        &file,
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

/// The file a profile's `image` names, as the media code needs it, or `None` when it is not an
/// image file Nexus holds. The address is read by [`FileRef::parse_uri`], the owner taken from
/// it, so another user's file is served from that user's folder. A v0 file is read from its
/// `File` node, as before, whatever its type. A `social/v1` file has no node: its type is the one
/// its extension names, which must be an image, and its `main` must be on disk, checked here so
/// a missing file starts no conversion.
async fn avatar_file(image_uri: &str, files_path: &FsPath) -> Result<Option<MediaFile>> {
    let Some((owner_id, file_ref)) = FileRef::parse_uri(image_uri) else {
        return Ok(None);
    };
    match file_ref {
        FileRef::Legacy(file_id) => {
            let files = FileDetails::get_by_ids(&[&[owner_id.as_ref(), &file_id]]).await?;
            Ok(files
                .into_iter()
                .flatten()
                .next()
                .as_ref()
                .map(MediaFile::from))
        }
        FileRef::V1 { filename, ext } => {
            let content_type = content_type_for_ext(&ext);
            if MediaKind::from_content_type(content_type) != Some(MediaKind::Image) {
                return Ok(None);
            }
            let file = MediaFile {
                owner_id: owner_id.to_string(),
                id: filename,
                content_type: content_type.to_string(),
                download_name: None,
            };
            Ok(v1_file_is_on_disk(files_path, &file).await?.then_some(file))
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(user_avatar_handler, v1_user_avatar_handler),
    components(schemas(PubkyId))
)]
pub struct UserAvatarApiDoc;
