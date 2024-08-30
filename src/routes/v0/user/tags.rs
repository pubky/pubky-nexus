use crate::models::tag::traits::TagCollection;
use crate::models::tag::user::TagUser;
use crate::models::tag::TagDetails;
use crate::routes::v0::endpoints::USER_TAGS_ROUTE;
use crate::routes::v0::TagsQuery;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use log::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = USER_TAGS_ROUTE,
    tag = "User Tags",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("limit_tags" = Option<usize>, Query, description = "Upper limit on the number of tags for the user"),
        ("limit_taggers" = Option<usize>, Query, description = "Upper limit on the number of taggers per tag")
    ),
    responses(
        (status = 200, description = "User tags", body = UserTags),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_tags_handler(
    Path(user_id): Path<String>,
    Query(query): Query<TagsQuery>,
) -> Result<Json<Vec<TagDetails>>> {
    info!(
        "GET {USER_TAGS_ROUTE} user_id:{}, limit_tags:{:?}, limit_taggers:{:?}",
        user_id, query.limit_tags, query.limit_taggers
    );

    match TagUser::get_by_id(&user_id, None, query.limit_tags, query.limit_taggers).await {
        Ok(Some(tags)) => Ok(Json(tags)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(user_tags_handler), components(schemas(TagDetails)))]
pub struct UserTagsApiDoc;
