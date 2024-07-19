use crate::models::profile::Profile;
use axum::extract::{Path, Query};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ProfileQuery {
    viewer_id: Option<String>,
}

#[utoipa::path(
    get,
    path = "/v0/profiles/{user_id}",
    params(
        ("user_id" = String, Path, description = "User ID"),
        ("viewer_id" = Option<String>, Query, description = "Viewer ID")
    ),
    responses(
        (status = 200, description = "User profile", body = Profile),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_profile(
    Path(user_id): Path<String>,
    Query(query): Query<ProfileQuery>,
) -> Result<Json<Profile>, Response> {
    match Profile::get_by_id(&user_id, query.viewer_id.as_deref()).await {
        Ok(Some(profile)) => Ok(Json(profile)),
        Ok(None) => Err((axum::http::StatusCode::NOT_FOUND, "User not found").into_response()),
        Err(_) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error",
        )
            .into_response()),
    }
}
