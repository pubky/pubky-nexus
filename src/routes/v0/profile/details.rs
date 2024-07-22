use crate::models::profile::{ProfileDetails, ProfileLink};
use crate::routes::v0::endpoints::PROFILE_DETAILS_ROUTE;
use crate::{Error, Result};
use axum::extract::Path;
use axum::Json;
use log::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = PROFILE_DETAILS_ROUTE,
    tag = "Profile Details",
    params(
        ("user_id" = String, Path, description = "User Pubky ID")
    ),
    responses(
        (status = 200, description = "User details", body = ProfileDetails),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn profile_details_handler(Path(user_id): Path<String>) -> Result<Json<ProfileDetails>> {
    info!("GET {PROFILE_DETAILS_ROUTE} user_id:{}", user_id);

    match ProfileDetails::get_by_id(&user_id).await {
        Ok(Some(details)) => Ok(Json(details)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(profile_details_handler),
    components(schemas(ProfileDetails, ProfileLink))
)]
pub struct ProfileDetailsApiDoc;
