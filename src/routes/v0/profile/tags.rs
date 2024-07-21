use axum::extract::Path;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use log::{error, info};
use utoipa::OpenApi;

use crate::models::profile::{ProfileTag, ProfileTags};
use crate::routes::v0::endpoints::PROFILE_TAGS_ROUTE;

#[utoipa::path(
    get,
    path = PROFILE_TAGS_ROUTE,
    tag = "Profile Tags",
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User tags", body = ProfileTags),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn profile_tags_handler(
    Path(user_id): Path<String>,
) -> Result<Json<ProfileTags>, Response> {
    info!("GET {PROFILE_TAGS_ROUTE} user_id:{}", user_id);

    match ProfileTags::get_by_id(&user_id).await {
        Ok(tags) => Ok(Json(tags)),
        Err(e) => {
            error!(
                "Internal server error while fetching profile tags for user_id: {}: {:?}",
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
    Router::new().route(PROFILE_TAGS_ROUTE, get(profile_tags_handler))
}

#[derive(OpenApi)]
#[openapi(
    paths(profile_tags_handler),
    components(schemas(ProfileTags, ProfileTag))
)]
pub struct ProfileTagsApiDoc;
