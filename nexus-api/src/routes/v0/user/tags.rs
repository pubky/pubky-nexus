use crate::routes::v0::endpoints::{USER_TAGGERS_ROUTE, USER_TAGS_ROUTE};
use crate::routes::v0::utils::json_array_or_no_content;
use crate::routes::v0::{TaggersInfoResponse, TagsQuery};
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use nexus_common::models::tag::traits::{TagCollection, TaggersCollection};
use nexus_common::models::tag::user::TagUser;
use nexus_common::models::tag::TagDetails;
use nexus_common::types::Pagination;
use serde::Deserialize;
use tracing::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = USER_TAGS_ROUTE,
    description = "User Tags",
    tag = "User",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("skip_tags" = Option<usize>, Query, description = "Skip N tags. **Default** value 0"),
        ("limit_tags" = Option<usize>, Query, description = "Upper limit on the number of tags for the user. **Default** value 5"),
        ("limit_taggers" = Option<usize>, Query, description = "Upper limit on the number of taggers per tag. **Default** value 5"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("depth" = Option<usize>, Query, description = "User trusted network depth, user following users distance. Numbers bigger than 4, will be ignored")
    ),
    responses(
        (status = 200, description = "User tags", body = TagDetails),
        (status = 204, description = "Tags not found"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_tags_handler(
    Path(user_id): Path<String>,
    Query(query): Query<TagsQuery>,
) -> Result<Json<Vec<TagDetails>>> {
    info!(
        "GET {USER_TAGS_ROUTE} user_id:{}, skip_tags:{:?}, limit_tags:{:?}, limit_taggers:{:?}, viewer_id:{:?}, depth:{:?}",
        user_id, query.skip_tags, query.limit_tags, query.limit_taggers, query.viewer_id, query.depth
    );

    match TagUser::get_by_id(
        &user_id,
        None,
        query.skip_tags,
        query.limit_tags,
        query.limit_taggers,
        query.viewer_id.as_deref(),
        query.depth,
    )
    .await
    {
        Ok(Some(tags)) => json_array_or_no_content(tags, "tags"),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(Deserialize)]
pub struct TaggersQuery {
    #[serde(flatten)]
    pub pagination: Pagination,
    #[serde(flatten)]
    pub tags_query: TagsQuery,
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
        ("limit" = Option<usize>, Query, description = "Number of taggers to return for pagination"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("depth" = Option<usize>, Query, description = "User trusted network depth, user following users distance. Numbers bigger than 4, will be ignored")
    ),
    responses(
        (status = 200, description = "User tags", body = TaggersInfoResponse),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_taggers_handler(
    Path((user_id, label)): Path<(String, String)>,
    Query(TaggersQuery {
        pagination,
        tags_query,
    }): Query<TaggersQuery>,
) -> Result<Json<TaggersInfoResponse>> {
    info!(
        "GET {USER_TAGGERS_ROUTE} user_id:{}, label: {}, skip:{:?}, limit:{:?}, viewer_id:{:?}, depth:{:?}",
        user_id, label, pagination.skip, pagination.limit, tags_query.viewer_id, tags_query.depth
    );

    match TagUser::get_tagger_by_id(
        &user_id,
        None,
        &label,
        pagination,
        tags_query.viewer_id.as_deref(),
        tags_query.depth,
    )
    .await
    {
        Ok(Some(tags)) => Ok(Json(TaggersInfoResponse::from(tags))),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(user_tags_handler, user_taggers_handler),
    components(schemas(TagDetails, TaggersInfoResponse))
)]
pub struct UserTagsApiDoc;
