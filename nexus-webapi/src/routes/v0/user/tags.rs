use crate::models::{BoundedLimit, BoundedPagination, BoundedSkip, PubkyId};
use crate::routes::v0::endpoints::{USER_TAGGERS_ROUTE, USER_TAGS_ROUTE};
use crate::routes::v0::types::resolve_tag_wot_depth;
use crate::routes::v0::version::{versioned_handlers, ApiVersion};
use crate::routes::v0::TagsQuery;
use crate::routes::Path;
use crate::routes::Query;
use crate::{Error, Result};
use axum::Json;
use nexus_common::models::tag::traits::{TagCollection, TaggersCollection};
use nexus_common::models::tag::user::TagUser;
use serde::Deserialize;
use tracing::debug;
use utoipa::OpenApi;

/// The one body behind `user_tags_handler_v0` and `user_tags_handler_v1`: the tags in the
/// version's shape.
pub async fn user_tags<V: ApiVersion>(
    Path(user_id): Path<PubkyId>,
    Query(query): Query<TagsQuery>,
) -> Result<Json<Vec<V::TagDetails>>> {
    debug!(
        "GET {USER_TAGS_ROUTE} user_id:{}, skip_tags:{:?}, limit_tags:{:?}, limit_taggers:{:?}, viewer_id:{:?}, depth:{:?}",
        user_id, query.skip_tags, query.limit_tags, query.limit_taggers, query.viewer_id, query.depth
    );

    let depth = resolve_tag_wot_depth(query.viewer_id.as_deref(), query.depth)?;
    match TagUser::get_by_id(
        &user_id,
        None,
        query.skip_tags_as_usize(),
        query.limit_tags_as_usize(),
        query.limit_taggers_as_usize(),
        query.viewer_id.as_deref(),
        depth,
    )
    .await?
    {
        Some(tags) => Ok(Json(tags.into_iter().map(V::TagDetails::from).collect())),
        None => Err(Error::user_not_found(user_id)),
    }
}

versioned_handlers! {
    user_tags_handler_v0 / user_tags_handler_v1 => user_tags(path: Path<PubkyId>, query: Query<TagsQuery>) -> Vec<shapes::TagDetails>;
    #[utoipa::path(
        get,
        path = USER_TAGS_ROUTE,
        operation_id = "user_tags_handler",
        description = "User Tags",
        tag = "User",
        params(
            ("user_id" = PubkyId, Path, description = "User Pubky ID"),
            ("skip_tags" = Option<BoundedSkip<10_000>>, Query, description = "Skip N tags (0–10 000, **default** 0)"),
            ("limit_tags" = Option<BoundedLimit<5, 100>>, Query, description = "Upper limit on the number of tags for the user (1–100, **default** 5)"),
            ("limit_taggers" = Option<BoundedLimit<5, 100>>, Query, description = "Upper limit on the number of taggers per tag (1–100, **default** 5)"),
            ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID"),
            ("depth" = Option<u8>, Query, description = "WoT depth (1-3). When provided with `viewer_id`, tags are filtered through the viewer's Web of Trust. `depth` without `viewer_id`, or an out-of-range `depth` with `viewer_id`, is rejected with 400.")
        )
    )]
    responses(
        (status = 200, description = "User tags"),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "User not found"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
}

/// The path of `GET /user/{user_id}/taggers/{label}`, with `label` folded as version `V`
/// folds it.
#[derive(Deserialize)]
#[serde(bound(deserialize = ""))]
pub struct UserTaggersPath<V: ApiVersion> {
    pub user_id: PubkyId,
    pub label: V::TagLabel,
}

#[derive(Deserialize)]
pub struct TaggersQuery {
    #[serde(flatten)]
    pub pagination: BoundedPagination<10_000, 40, 100>,
    #[serde(flatten)]
    pub tags_query: TagsQuery,
}

/// The one body behind `user_taggers_handler_v0` and `user_taggers_handler_v1`: the taggers in
/// the version's shape, of the label as the version folds it.
pub async fn user_taggers<V: ApiVersion>(
    Path(UserTaggersPath { user_id, label }): Path<UserTaggersPath<V>>,
    Query(taggers_query): Query<TaggersQuery>,
) -> Result<Json<V::TaggersInfoResponse>> {
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
    Ok(Json(V::TaggersInfoResponse::from(taggers)))
}

versioned_handlers! {
    user_taggers_handler_v0 / user_taggers_handler_v1 => user_taggers(path: Path<shapes::UserTaggersPath>, query: Query<TaggersQuery>) -> shapes::TaggersInfoResponse;
    #[utoipa::path(
        get,
        path = USER_TAGGERS_ROUTE,
        operation_id = "user_taggers_handler",
        description = "User label taggers",
        tag = "User",
        params(
            ("user_id" = PubkyId, Path, description = "User Pubky ID"),
            ("label" = shapes::TagLabel, Path, description = "Tag name"),
            ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Skip N taggers (0–10 000, **default** 0)"),
            ("limit" = Option<BoundedLimit<40, 100>>, Query, description = "Number of taggers to return for pagination (1–100, **default** 40)"),
            ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID"),
            ("depth" = Option<u8>, Query, description = "WoT depth (1-3). When provided with `viewer_id`, taggers are filtered through the viewer's Web of Trust. `depth` without `viewer_id`, or an out-of-range `depth` with `viewer_id`, is rejected with 400.")
        )
    )]
    responses(
        (status = 200, description = "User tags"),
        (status = 400, description = "Invalid parameters"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
}

/// The schemas the user tag routes share; the routes themselves, whose tags, taggers and label
/// are the version's, come with the version's document.
#[derive(OpenApi)]
#[openapi(components(schemas(PubkyId)))]
pub struct UserTagsApiDoc;
