use crate::models::profile::{ProfileTag, ProfileTags};
use crate::routes::v0::endpoints::PROFILE_TAGS_ROUTE;
use crate::{Error, Result};
use axum::extract::Path;
use axum::Json;
use log::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = PROFILE_TAGS_ROUTE,
    tag = "Profile Tags",
    params(
        ("user_id" = String, Path, description = "User Pubky ID")
    ),
    responses(
        (status = 200, description = "User tags", body = ProfileTags),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn profile_tags_handler(Path(user_id): Path<String>) -> Result<Json<ProfileTags>> {
    info!("GET {PROFILE_TAGS_ROUTE} user_id:{}", user_id);

    match ProfileTags::get_by_id(&user_id).await {
        Ok(Some(tags)) => Ok(Json(tags)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(profile_tags_handler),
    components(schemas(ProfileTags, ProfileTag))
)]
pub struct ProfileTagsApiDoc;
