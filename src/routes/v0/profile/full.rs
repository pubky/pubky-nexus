use axum::extract::{Path, Query};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use log::{debug, error, info};
use serde::Deserialize;
use utoipa::OpenApi;

use crate::models::profile::ProfileView;
use crate::routes::v0::endpoints::PROFILE_ROUTE;

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
) -> Result<Json<ProfileView>, Response> {
    info!(
        "GET {PROFILE_ROUTE} user_id:{}, viewer_id:{:?}",
        user_id, query.viewer_id
    );

    match ProfileView::get_by_id(&user_id, query.viewer_id.as_deref()).await {
        Ok(Some(profile)) => Ok(Json(profile)),
        Ok(None) => {
            debug!("Profile not found for user_id: {}", user_id);
            Err((axum::http::StatusCode::NOT_FOUND, "User not found").into_response())
        }
        Err(e) => {
            error!(
                "Internal server error while fetching profile for user_id: {}: {:?}",
                user_id, e
            );
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error",
            )
                .into_response())
        }
    }
}

pub fn route() -> Router {
    Router::new().route(PROFILE_ROUTE, get(profile_full_view_handler))
}

#[derive(OpenApi)]
#[openapi(paths(profile_full_view_handler), components(schemas(ProfileView)))]
pub struct ProfileViewApiDoc;
