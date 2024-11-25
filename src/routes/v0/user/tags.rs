use crate::models::tag::traits::taggers::Taggers;
use crate::models::tag::traits::{TagCollection, TaggersCollection};
use crate::models::tag::user::TagUser;
use crate::models::tag::TagDetails;
use crate::models::user::{ProfileTag, UserTags};
use crate::routes::v0::endpoints::{USER_TAGGERS_ROUTE, USER_TAGS_ROUTE};
use crate::routes::v0::TagsQuery;
use crate::types::Pagination;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use log::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = USER_TAGS_ROUTE,
    description = "User Tags",
    tag = "User",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("limit_tags" = Option<usize>, Query, description = "Upper limit on the number of tags for the user"),
        ("limit_taggers" = Option<usize>, Query, description = "Upper limit on the number of taggers per tag"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("depth" = Option<usize>, Query, description = "User trusted network depth, user following users distance. Numbers bigger than 4, will be ignored")
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
        "GET {USER_TAGS_ROUTE} user_id:{}, limit_tags:{:?}, limit_taggers:{:?}, viewer_id:{:?}, depth:{:?}",
        user_id, query.limit_tags, query.limit_taggers, query.viewer_id, query.depth
    );

    match TagUser::get_by_id(&user_id, None, query.limit_tags, query.limit_taggers, query.viewer_id.as_deref(), query.depth).await {
        Ok(Some(tags)) => Ok(Json(tags)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[utoipa::path(
    get,
    path = USER_TAGGERS_ROUTE,
    description = "User label taggers",
    tag = "User",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("label" = String, Path, description = "Tag name"),
        ("skip" = Option<usize>, Query, description = "Number of taggers to skip for pagination"),
        ("limit" = Option<usize>, Query, description = "Number of taggers to return for pagination")
    ),
    responses(
        (status = 200, description = "User tags", body = UserTags),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_taggers_handler(
    Path((user_id, label)): Path<(String, String)>,
    Query(pagination): Query<Pagination>,
) -> Result<Json<Taggers>> {
    info!(
        "GET {USER_TAGGERS_ROUTE} user_id:{}, label: {}, skip:{:?}, limit:{:?}",
        user_id, label, pagination.skip, pagination.limit
    );

    match TagUser::get_tagger_by_id(&user_id, None, &label, pagination).await {
        Ok(Some(tags)) => Ok(Json(tags)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(user_tags_handler, user_taggers_handler),
    components(schemas(TagDetails, UserTags, ProfileTag))
)]
pub struct UserTagsApiDoc;
