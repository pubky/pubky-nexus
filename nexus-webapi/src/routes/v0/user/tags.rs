use crate::models::{BoundedLimit, BoundedPagination, BoundedSkip, PubkyId, TagLabel};
use crate::routes::v0::endpoints::{USER_TAGGERS_ROUTE, USER_TAGS_ROUTE};
use crate::routes::v0::types::resolve_tag_wot_depth;
use crate::routes::v0::{TaggersInfoResponse, TagsQuery};
use crate::routes::Path;
use crate::routes::Query;
use crate::{Error, Result};
use axum::Json;
use nexus_common::models::tag::traits::{TagCollection, TaggersCollection};
use nexus_common::models::tag::user::TagUser;
use nexus_common::models::tag::TagDetails;
use serde::Deserialize;
use tracing::debug;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = USER_TAGS_ROUTE,
    description = "User Tags",
    tag = "User",
    params(
        ("user_id" = PubkyId, Path, description = "User Pubky ID"),
        ("skip_tags" = Option<BoundedSkip<10_000>>, Query, description = "Skip N tags. **Default** value 0"),
        ("limit_tags" = Option<BoundedLimit<5, 100>>, Query, description = "Upper limit on the number of tags for the user (1–100, **default** 5)"),
        ("limit_taggers" = Option<BoundedLimit<5, 100>>, Query, description = "Upper limit on the number of taggers per tag (1–100, **default** 5)"),
        ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID"),
        ("depth" = Option<u8>, Query, description = "WoT depth (1-3). When provided with `viewer_id`, tags are filtered through the viewer's Web of Trust. `depth` without `viewer_id`, or an out-of-range `depth` with `viewer_id`, is rejected with 400.")
    ),
    responses(
        (status = 200, description = "User tags", body = Vec<TagDetails>),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_tags_handler(
    Path(user_id): Path<PubkyId>,
    Query(query): Query<TagsQuery>,
) -> Result<Json<Vec<TagDetails>>> {
    debug!(
        "GET {USER_TAGS_ROUTE} user_id:{}, skip_tags:{:?}, limit_tags:{:?}, limit_taggers:{:?}, viewer_id:{:?}, depth:{:?}",
        user_id, query.skip_tags, query.limit_tags, query.limit_taggers, query.viewer_id, query.depth
    );

    let depth = resolve_tag_wot_depth(query.viewer_id.as_deref(), query.depth)?;
    match TagUser::get_by_id(
        &user_id,
        None,
        query.skip_tags.map(|s| s.value()),
        query.limit_tags.map(|l| l.value()),
        query.limit_taggers.map(|l| l.value()),
        query.viewer_id.as_deref(),
        depth,
    )
    .await?
    {
        Some(tags) => Ok(Json(tags)),
        None => Err(Error::user_not_found(user_id)),
    }
}

#[derive(Deserialize)]
pub struct UserTaggersPath {
    pub user_id: PubkyId,
    pub label: TagLabel,
}

#[derive(Deserialize)]
pub struct TaggersQuery {
    #[serde(flatten)]
    pub pagination: BoundedPagination<10_000, 40, 100>,
    #[serde(flatten)]
    pub tags_query: TagsQuery,
}

#[utoipa::path(
    get,
    path = USER_TAGGERS_ROUTE,
    description = "User label taggers",
    tag = "User",
    params(
        ("user_id" = PubkyId, Path, description = "User Pubky ID"),
        ("label" = TagLabel, Path, description = "Tag name"),
        ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Number of taggers to skip for pagination"),
        ("limit" = Option<BoundedLimit<40, 100>>, Query, description = "Number of taggers to return for pagination (1–100, **default** 40)"),
        ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID"),
        ("depth" = Option<u8>, Query, description = "WoT depth (1-3). When provided with `viewer_id`, taggers are filtered through the viewer's Web of Trust. `depth` without `viewer_id`, or an out-of-range `depth` with `viewer_id`, is rejected with 400.")
    ),
    responses(
        (status = 200, description = "User tags", body = TaggersInfoResponse),
        (status = 400, description = "Invalid parameters"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_taggers_handler(
    Path(UserTaggersPath { user_id, label }): Path<UserTaggersPath>,
    Query(taggers_query): Query<TaggersQuery>,
) -> Result<Json<TaggersInfoResponse>> {
    debug!(
        "GET {USER_TAGGERS_ROUTE} user_id:{}, label: {}, skip:{}, limit:{}, viewer_id:{:?}, depth:{:?}",
        user_id,
        label,
        taggers_query.pagination.skip_value(),
        taggers_query.pagination.limit_value(),
        taggers_query.tags_query.viewer_id,
        taggers_query.tags_query.depth
    );

    let pagination = taggers_query.pagination.to_pagination(None, None);

    let depth = resolve_tag_wot_depth(
        taggers_query.tags_query.viewer_id.as_deref(),
        taggers_query.tags_query.depth,
    )?;
    let taggers = TagUser::get_tagger_by_id(
        &user_id,
        None,
        &label,
        pagination,
        taggers_query.tags_query.viewer_id.as_deref(),
        depth,
    )
    .await?;
    Ok(Json(TaggersInfoResponse::from(taggers)))
}

#[derive(OpenApi)]
#[openapi(
    paths(user_tags_handler, user_taggers_handler),
    components(schemas(TagDetails, TaggersInfoResponse, PubkyId, TagLabel))
)]
pub struct UserTagsApiDoc;
