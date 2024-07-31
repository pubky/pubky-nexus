use crate::models::tag::user::{UserTag, UserTags};
use crate::models::tag::{Tag, Tags};
use crate::routes::v0::endpoints::USER_TAGS_ROUTE;
use crate::{Error, Result};
use axum::extract::Path;
use axum::Json;
use log::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = USER_TAGS_ROUTE,
    tag = "User Tags",
    params(
        ("user_id" = String, Path, description = "User Pubky ID")
    ),
    responses(
        (status = 200, description = "User tags", body = UserTags),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_tags_handler(Path(user_id): Path<String>) -> Result<Json<UserTags>> {
    info!("GET {USER_TAGS_ROUTE} user_id:{}", user_id);

    match UserTags::get_by_id(&user_id).await {
        Ok(Some(tags)) => Ok(Json(tags)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(user_tags_handler),
    components(schemas(UserTags, UserTag, Tags, Tag))
)]
pub struct UserTagsApiDoc;
