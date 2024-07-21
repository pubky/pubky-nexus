use axum::extract::Path;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use log::{debug, error, info};
use utoipa::OpenApi;

use crate::models::profile::ProfileCounts;
use crate::routes::v0::endpoints::PROFILE_COUNTS_ROUTE;

#[utoipa::path(
    get,
    path = PROFILE_COUNTS_ROUTE,
    tag = "Profile Counts",
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User counts", body = ProfileCounts),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn profile_counts_handler(
    Path(user_id): Path<String>,
) -> Result<Json<ProfileCounts>, Response> {
    info!("GET {PROFILE_COUNTS_ROUTE} user_id:{}", user_id);

    match ProfileCounts::get_by_id(&user_id).await {
        Ok(Some(counts)) => Ok(Json(counts)),
        Ok(None) => {
            debug!("User not found for counts of user_id: {}", user_id);
            Err((axum::http::StatusCode::NOT_FOUND, "User not found").into_response())
        }
        Err(e) => {
            error!(
                "Internal server error while fetching profile counts for user_id: {}: {:?}",
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
    Router::new().route(PROFILE_COUNTS_ROUTE, get(profile_counts_handler))
}

#[derive(OpenApi)]
#[openapi(paths(profile_counts_handler), components(schemas(ProfileCounts)))]
pub struct ProfileCountsApiDoc;
