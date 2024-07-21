use axum::extract::Path;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use log::{debug, error, info};
use utoipa::OpenApi;

use crate::models::profile::{ProfileDetails, ProfileLink};
use crate::routes::v0::endpoints::PROFILE_DETAILS_ROUTE;

#[utoipa::path(
    get,
    path = PROFILE_DETAILS_ROUTE,
    tag = "Profile Details",
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User details", body = ProfileDetails),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn profile_details_handler(
    Path(user_id): Path<String>,
) -> Result<Json<ProfileDetails>, Response> {
    info!("GET {PROFILE_DETAILS_ROUTE} user_id:{}", user_id);

    match ProfileDetails::get_by_id(&user_id).await {
        Ok(Some(details)) => Ok(Json(details)),
        Ok(None) => {
            debug!("Profile details not found for user_id: {}", user_id);
            Err((axum::http::StatusCode::NOT_FOUND, "User not found").into_response())
        }
        Err(e) => {
            error!(
                "Internal server error while fetching profile details for user_id: {}: {:?}",
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
    Router::new().route(PROFILE_DETAILS_ROUTE, get(profile_details_handler))
}

#[derive(OpenApi)]
#[openapi(
    paths(profile_details_handler),
    components(schemas(ProfileDetails, ProfileLink))
)]
pub struct ProfileDetailsApiDoc;
