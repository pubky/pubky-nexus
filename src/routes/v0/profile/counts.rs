use axum::extract::Path;
use axum::Json;
use log::info;
use utoipa::OpenApi;

use crate::models::profile::ProfileCounts;
use crate::routes::v0::endpoints::PROFILE_COUNTS_ROUTE;
use crate::{Error, Result};

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
pub async fn profile_counts_handler(Path(user_id): Path<String>) -> Result<Json<ProfileCounts>> {
    info!("GET {PROFILE_COUNTS_ROUTE} user_id:{}", user_id);

    match ProfileCounts::get_by_id(&user_id).await {
        Ok(Some(counts)) => Ok(Json(counts)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(profile_counts_handler), components(schemas(ProfileCounts)))]
pub struct ProfileCountsApiDoc;
