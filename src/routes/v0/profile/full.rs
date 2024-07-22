use axum::extract::{Path, Query};
use axum::Json;
use log::info;
use serde::Deserialize;
use utoipa::OpenApi;

use crate::models::profile::ProfileView;
use crate::routes::v0::endpoints::PROFILE_ROUTE;
use crate::{Error, Result};

#[derive(Deserialize)]
pub struct ProfileQuery {
    viewer_id: Option<String>,
}

#[utoipa::path(
    get,
    path = PROFILE_ROUTE,
    tag = "Profile Full View",
    params(
        ("user_id" = String, Path, description = "User ID"),
        ("viewer_id" = Option<String>, Query, description = "Viewer ID")
    ),
    responses(
        (status = 200, description = "User profile", body = ProfileView),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn profile_full_view_handler(
    Path(user_id): Path<String>,
    Query(query): Query<ProfileQuery>,
) -> Result<Json<ProfileView>> {
    info!(
        "GET {PROFILE_ROUTE} user_id:{}, viewer_id:{:?}",
        user_id, query.viewer_id
    );

    match ProfileView::get_by_id(&user_id, query.viewer_id.as_deref()).await {
        Ok(Some(profile)) => Ok(Json(profile)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(profile_full_view_handler), components(schemas(ProfileView)))]
pub struct ProfileViewApiDoc;
