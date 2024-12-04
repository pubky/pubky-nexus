use crate::models::tag::global::TagGlobal;
use crate::models::tag::stream::{
    HotTag, HotTags, HotTagsInput, TagStreamReach, TaggedType, Taggers,
};
use crate::routes::v0::endpoints::{TAGS_HOT_ROUTE, TAG_TAGGERS_ROUTE};
use crate::types::{Pagination, Timeframe};
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use log::{error, info};
use serde::Deserialize;
use utoipa::OpenApi;

#[derive(Deserialize, Debug)]
pub struct HotTagsQuery {
    user_id: Option<String>,
    reach: Option<TagStreamReach>,
    taggers_limit: Option<usize>,
    timeframe: Option<Timeframe>,

    #[serde(flatten)]
    pagination: Pagination,
}

#[derive(Deserialize, Debug)]
pub struct TagTaggersQuery {
    pagination: Pagination,
    user_id: Option<String>,
    reach: Option<TagStreamReach>,
}

#[utoipa::path(
    get,
    path = TAG_TAGGERS_ROUTE,
    description = "Global tag Taggers",
    tag = "Tags",
    params(
        ("label" = String, Path, description = "Tag name"),
        ("reach" = TagStreamReach, Path, description = "Reach type: Follower | Following | Friends"),
        ("user_id" = Option<String>, Query, description = "User ID to base reach on"),
    ),
    responses(
        (status = 200, description = "Taggers", body = Vec<String>),
        (status = 404, description = "Tag not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn tag_taggers_handler(
    Path(label): Path<String>,
    Query(query): Query<TagTaggersQuery>,
) -> Result<Json<Vec<String>>> {
    info!(
        "GET {TAG_TAGGERS_ROUTE} label:{}, query: {:?}",
        label, query
    );

    let skip = query.pagination.skip.unwrap_or(0);
    let limit = query.pagination.limit.unwrap_or(20).min(20);

    match TagGlobal::get_tag_taggers(label.clone(), query.user_id, query.reach, skip, limit).await {
        Ok(Some(post)) => Ok(Json(post)),
        Ok(None) => Err(Error::TagsNotFound { reach: label }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[utoipa::path(
    get,
    path = TAGS_HOT_ROUTE,
    description = "Global Tags by reach",
    tag = "Tags",
    params(
        ("user_id" = Option<String>, Query, description = "User Pubky ID"),
        ("reach" = Option<TagStreamReach>, Query, description = "Reach type: follower | following | friends | wot"),
        ("taggers_limit" = Option<usize>, Query, description = "Retrieve N user_id for each tag"),
        ("skip" = Option<usize>, Query, description = "Skip N tags"),
        ("limit" = Option<usize>, Query, description = "Retrieve N tag"),
        ("timeframe" = Option<Timeframe>, Query, description = "Retrieve hot tags for this specific timeframe. Defaults to all_time"),
    ),
    responses(
        (status = 200, description = "Retrieve tags by reach cluster", body = Vec<HotTag>),
        (status = 404, description = "Hot tags not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn hot_tags_handler(Query(query): Query<HotTagsQuery>) -> Result<Json<HotTags>> {
    info!("GET {TAGS_HOT_ROUTE}, query: {:?}", query);

    // Check if user_id and reach are provided together
    if query.user_id.is_some() ^ query.reach.is_some() {
        return Err(Error::InvalidInput {
            message: String::from("user_id and reach should be both provided together"),
        });
    }

    let skip = query.pagination.skip.unwrap_or(0);
    let limit = query.pagination.limit.unwrap_or(40).min(40);
    let taggers_limit = query.taggers_limit.unwrap_or(20).min(20);
    let timeframe = query.timeframe.unwrap_or(Timeframe::AllTime);

    let input = HotTagsInput {
        timeframe,
        skip,
        limit,
        taggers_limit,
        tagged_type: Some(TaggedType::Post),
    };

    match HotTags::get_hot_tags(query.user_id, query.reach, &input).await {
        Ok(Some(hot_tags)) => Ok(Json(hot_tags)),
        Ok(None) => Ok(Json(HotTags(vec![]))),
        Err(source) => {
            error!("Internal Server ERROR: {:?}", source);
            Err(Error::InternalServerError { source })
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(hot_tags_handler, tag_taggers_handler),
    components(schemas(HotTags, HotTag, Taggers))
)]
pub struct TagGlobalApiDoc;
