use crate::models::tag::global::TagGlobal;
use crate::models::tag::stream::{HotTag, HotTags, TagStreamReach, Taggers};
use crate::routes::v0::endpoints::{TAG_HOT_ROUTE, TAG_REACH_ROUTE, TAG_TAGGERS_ROUTE};
use crate::types::Pagination;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use log::{error, info};
use serde::Deserialize;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct HotTagsQuery {
    max_taggers: Option<usize>,
    #[serde(flatten)]
    pagination: Pagination,
}

#[utoipa::path(
    get,
    path = TAG_HOT_ROUTE,
    params(
        ("max_taggers" = Option<usize>, Query, description = "Retrieve N user_id for each tag"),
        ("skip" = Option<usize>, Query, description = "Skip N tags"),
        ("limit" = Option<usize>, Query, description = "Retrieve N tag")
    ),
    tag = "Tags",
    description = "Global hot Tags",
    responses(
        // TODO: Add hot tags
        (status = 200, description = "Retrieve hot tags", body = Vec<HotTag>),
        (status = 404, description = "Hot tags not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn hot_tags_handler(Query(query): Query<HotTagsQuery>) -> Result<Json<HotTags>> {
    info!(
        "GET {TAG_HOT_ROUTE} skip:{:?}, limit:{:?}, max_tagger: {:?}",
        query.pagination.skip, query.pagination.limit, query.max_taggers
    );

    let skip = query.pagination.skip.unwrap_or(0);
    let limit = query.pagination.limit.unwrap_or(40);
    let max_taggers = query.max_taggers.unwrap_or(20);

    match HotTags::get_global_tags_stream(Some(skip), Some(limit), Some(max_taggers)).await {
        Ok(Some(hot_tags)) => Ok(Json(hot_tags)),
        Ok(None) => Err(Error::TagsNotFound {
            reach: String::from("GLOBAL"),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(Deserialize)]
pub struct TagTaggersQuery {
    reach: Option<TagStreamReach>,
}

#[utoipa::path(
    get,
    path = TAG_TAGGERS_ROUTE,
    description = "Global tag Taggers",
    tag = "Tags",
    params(
        ("label" = String, Path, description = "Tag name"),
        ("reach" = TagStreamReach, Path, description = "Reach type: Follower | Following | Friends")
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
        "GET {TAG_TAGGERS_ROUTE} label:{}, reach:{:?}",
        label, query.reach
    );

    let reach = query.reach;

    match TagGlobal::get_tag_taggers(label.clone(), reach).await {
        Ok(Some(post)) => Ok(Json(post)),
        Ok(None) => Err(Error::TagsNotFound { reach: label }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(Deserialize)]
pub struct TagsByReachQuery {
    user_id: String,
    reach: Option<TagStreamReach>,
}

#[utoipa::path(
    get,
    path = TAG_REACH_ROUTE,
    description = "Global Tags by reach",
    tag = "Tags",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("reach" = TagStreamReach, Path, description = "Reach type: Follower | Following | Friends")
    ),
    responses(
        (status = 200, description = "Retrieve tags by reach cluster", body = Vec<HotTag>),
        (status = 404, description = "Hot tags not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn tags_by_reach_handler(Path(path): Path<TagsByReachQuery>) -> Result<Json<HotTags>> {
    info!(
        "GET {TAG_REACH_ROUTE}: {:?}, {:?}",
        path.user_id, path.reach
    );

    let reach = path.reach.unwrap_or(TagStreamReach::Following);
    let user_id = path.user_id;

    match HotTags::get_stream_tags_by_reach(user_id, reach.clone()).await {
        Ok(Some(hot_tags)) => Ok(Json(hot_tags)),
        Ok(None) => Err(Error::TagsNotFound {
            reach: format!("{:?}", reach),
        }),
        Err(source) => {
            error!("Internal Server ERROR: {:?}", source);
            Err(Error::InternalServerError { source })
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(hot_tags_handler, tag_taggers_handler, tags_by_reach_handler),
    components(schemas(HotTags, HotTag, Taggers))
)]
pub struct TagGlobalApiDoc;
