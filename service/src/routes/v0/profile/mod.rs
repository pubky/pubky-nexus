use axum::extract::{Path, Query};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;

use crate::models::profile::{
    ProfileCounts, ProfileDetails, ProfileTags, ProfileView, Relationship,
};

use super::endpoints::{
    PROFILE_COUNTS_ROUTE, PROFILE_DETAILS_ROUTE, PROFILE_ROUTE, PROFILE_TAGS_ROUTE,
    RELATIONSHIP_ROUTE,
};

#[derive(Deserialize)]
pub struct ProfileQuery {
    viewer_id: Option<String>,
}

#[utoipa::path(
    get,
    path = PROFILE_ROUTE,
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
pub async fn get_profile(
    Path(user_id): Path<String>,
    Query(query): Query<ProfileQuery>,
) -> Result<Json<ProfileView>, Response> {
    match ProfileView::get_by_id(&user_id, query.viewer_id.as_deref()).await {
        Ok(Some(profile)) => Ok(Json(profile)),
        Ok(None) => Err((axum::http::StatusCode::NOT_FOUND, "User not found").into_response()),
        Err(_) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error",
        )
            .into_response()),
    }
}

#[utoipa::path(
    get,
    path = RELATIONSHIP_ROUTE,
    params(
        ("user_id" = String, Path, description = "User ID"),
        ("viewer_id" = String, Path, description = "Viewer ID")
    ),
    responses(
        (status = 200, description = "User relationship", body = Relationship),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_relationship(
    Path((user_id, viewer_id)): Path<(String, String)>,
) -> Result<Json<Relationship>, Response> {
    match Relationship::get_by_id(&user_id, Some(&viewer_id)).await {
        Ok(Some(relationship)) => Ok(Json(relationship)),
        Ok(None) => Err((axum::http::StatusCode::NOT_FOUND, "User not found").into_response()),
        Err(_) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error",
        )
            .into_response()),
    }
}

#[utoipa::path(
    get,
    path = PROFILE_COUNTS_ROUTE,
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User counts", body = Counts),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_counts(Path(user_id): Path<String>) -> Result<Json<ProfileCounts>, Response> {
    match ProfileCounts::get_by_id(&user_id).await {
        Ok(counts) => Ok(Json(counts)),
        Err(_) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error",
        )
            .into_response()),
    }
}

#[utoipa::path(
    get,
    path = PROFILE_DETAILS_ROUTE,
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User details", body = Details),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_details(Path(user_id): Path<String>) -> Result<Json<ProfileDetails>, Response> {
    match ProfileDetails::get_by_id(&user_id).await {
        Ok(Some(details)) => Ok(Json(details)),
        Ok(None) => Err((axum::http::StatusCode::NOT_FOUND, "User not found").into_response()),
        Err(_) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error",
        )
            .into_response()),
    }
}

#[utoipa::path(
    get,
    path = PROFILE_TAGS_ROUTE,
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User tags", body = ProfileTags),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_tags(Path(user_id): Path<String>) -> Result<Json<ProfileTags>, Response> {
    match ProfileTags::get_by_id(&user_id).await {
        Ok(tags) => Ok(Json(tags)),
        Err(_) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error",
        )
            .into_response()),
    }
}
