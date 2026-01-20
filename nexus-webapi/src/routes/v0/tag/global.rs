use crate::routes::v0::endpoints::{TAGS_HOT_ROUTE, TAG_TAGGERS_ROUTE};
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use nexus_common::models::tag::global::Taggers;
use nexus_common::models::tag::stream::{HotTag, HotTags};
use nexus_common::models::tag::TaggedType;
use nexus_common::models::tag::Taggers as TaggersType;
use nexus_common::types::routes::HotTagsInputDTO;
use nexus_common::types::{Pagination, StreamReach, Timeframe};
use serde::Deserialize;
use tracing::debug;
use utoipa::OpenApi;

#[derive(Deserialize, Debug)]
pub struct HotTagsQuery {
    user_id: Option<String>,
    reach: Option<StreamReach>,
    taggers_limit: Option<usize>,
    timeframe: Option<Timeframe>,
    #[serde(flatten)]
    pagination: Pagination,
}

#[derive(Deserialize, Debug)]
pub struct TagTaggersQuery {
    #[serde(flatten)]
    pagination: Pagination,
    user_id: Option<String>,
    reach: Option<StreamReach>,
    timeframe: Option<Timeframe>,
}

#[utoipa::path(
    get,
    path = TAG_TAGGERS_ROUTE,
    description = "Global tag Taggers",
    tag = "Tags",
    params(
        ("label" = String, Path, description = "Tag name"),
        ("reach" = Option<StreamReach>, Query, description = "Reach type: `follower` | `following` | `friends` | `wot`. To apply that, user_id is required"),
        ("user_id" = Option<String>, Query, description = "User ID to base reach on"),
        ("skip" = Option<usize>, Query, description = "Skip N taggers. Defaults to `0`"),
        ("limit" = Option<usize>, Query, description = "Retrieve N tagggers. Defaults to `20`"),
        ("timeframe" = Option<Timeframe>, Query, description = "Retrieve taggers for this specific timeframe (not applied for reach). Defaults to `all_time`"),
    ),
    responses(
        (status = 200, description = "Taggers", body = TaggersType),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn tag_taggers_handler(
    Path(label): Path<String>,
    Query(query): Query<TagTaggersQuery>,
) -> Result<Json<TaggersType>> {
    debug!("GET {TAG_TAGGERS_ROUTE} label:{label}, query: {query:?}");

    // Check if user_id and reach are provided together
    if query.user_id.is_some() ^ query.reach.is_some() {
        return Err(Error::invalid_input(
            "user_id and reach should be both provided together",
        ));
    }

    let skip = query.pagination.skip.unwrap_or(0);
    let limit = query.pagination.limit.unwrap_or(20).min(20);
    let timeframe = query.timeframe.unwrap_or(Timeframe::AllTime);

    match Taggers::get_global_taggers(
        label.clone(),
        query.user_id,
        query.reach,
        skip,
        limit,
        timeframe,
    )
    .await?
    {
        Some(post) => Ok(Json(post)),
        None => Ok(Json(vec![])),
    }
}

#[utoipa::path(
    get,
    path = TAGS_HOT_ROUTE,
    description = "Global Tags by reach",
    tag = "Tags",
    params(
        ("user_id" = Option<String>, Query, description = "User Pubky ID"),
        ("reach" = Option<StreamReach>, Query, description = "Reach type: `follower` | `following` | `friends` | `wot`. To apply that, user_id is required"),
        ("taggers_limit" = Option<usize>, Query, description = "Retrieve N user_id for each tag. Defaults to `20`"),
        ("skip" = Option<usize>, Query, description = "Skip N tags. Defaults to `0`"),
        ("limit" = Option<usize>, Query, description = "Retrieve N tag. Defaults to `40`"),
        ("timeframe" = Option<Timeframe>, Query, description = "Retrieve hot tags for this specific timeframe. Defaults to `all_time`"),
    ),
    responses(
        (status = 200, description = "Retrieve tags by reach cluster", body = Vec<HotTag>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn hot_tags_handler(Query(query): Query<HotTagsQuery>) -> Result<Json<HotTags>> {
    debug!("GET {TAGS_HOT_ROUTE}, query: {:?}", query);

    // Check if user_id and reach are provided together
    if query.user_id.is_some() ^ query.reach.is_some() {
        return Err(Error::invalid_input(
            "user_id and reach should be both provided together",
        ));
    }

    let skip = query.pagination.skip.unwrap_or(0);
    let limit = query.pagination.limit.unwrap_or(40).min(40);
    let taggers_limit = query.taggers_limit.unwrap_or(20).min(20);
    let timeframe = query.timeframe.unwrap_or(Timeframe::AllTime);

    let input = HotTagsInputDTO {
        timeframe,
        skip,
        limit,
        taggers_limit,
        tagged_type: Some(TaggedType::Post),
    };

    match HotTags::get_hot_tags(query.user_id, query.reach, &input).await? {
        Some(hot_tags) => Ok(Json(hot_tags)),
        None => Ok(Json(HotTags::default())),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(hot_tags_handler, tag_taggers_handler),
    components(schemas(HotTags, HotTag, Taggers, StreamReach, Timeframe))
)]
pub struct TagGlobalApiDoc;
