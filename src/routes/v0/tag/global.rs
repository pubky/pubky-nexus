use crate::models::tag::global::Global;
use crate::models::tag::stream::{HotTag, HotTags, Taggers};
use crate::models::user::UserStreamType;
use crate::routes::v0::endpoints::{TAG_HOT_ROUTE, TAG_POST_ROUTE, TAG_REACH_ROUTE};
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use log::{error, info};
use serde::Deserialize;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct HotTagsQuery {
    skip: Option<usize>,
    limit: Option<usize>,
    taggers: Option<usize>,
}

#[utoipa::path(
    get,
    path = TAG_HOT_ROUTE,
    params(
        ("skip" = Option<usize>, Query, description = "Skip N tags"),
        ("limit" = Option<usize>, Query, description = "Retrieve N tag"),
        ("taggers" = Option<usize>, Query, description = "Retrieve N user_id for each tag")
    ),
    tag = "Global hot Tags",
    responses(
        // TODO: Add hot tags
        (status = 200, description = "Retrieve hot tags", body = Vec<HotTag>),
        (status = 404, description = "Hot tags not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn hot_tags_handler(Query(query): Query<HotTagsQuery>) -> Result<Json<HotTags>> {
    info!("GET {TAG_HOT_ROUTE}");

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(40);
    let taggers_limit = query.taggers.unwrap_or(20);

    match HotTags::get_global_tags_stream(Some(skip), Some(limit), Some(taggers_limit)).await {
        Ok(Some(hot_tags)) => Ok(Json(hot_tags)),
        Ok(None) => Err(Error::TagsNotFound {
            reach: String::from("GLOBAL"),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(Deserialize)]
pub struct TagsByReachQuery {
    user_id: String,
    reach: Option<UserStreamType>,
}

#[utoipa::path(
    get,
    path = TAG_REACH_ROUTE,
    tag = "Global Tags by reach",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("reach" = UserStreamType, Path, description = "Reach type: Follower | Following | Friends")
    ),
    responses(
        (status = 200, description = "Retrieve tags by reach cluster", body = Vec<HotTag>),
        (status = 404, description = "Hot tags not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn tags_by_reach_handler(Path(path): Path<TagsByReachQuery>) -> Result<Json<HotTags>> {
    info!("GET {TAG_REACH_ROUTE}");

    let reach = path.reach.unwrap_or(UserStreamType::Following);
    let user_id = path.user_id;

    match HotTags::get_stream_tags_by_reach(user_id, reach).await {
        Ok(Some(hot_tags)) => Ok(Json(hot_tags)),
        Ok(None) => Err(Error::TagsNotFound {
            reach: String::from("REACH"),
        }),
        Err(source) => {
            error!("Internal Server ERROR: {:?}", source);
            Err(Error::InternalServerError { source })
        }
    }
}

#[derive(Deserialize)]
pub struct TagTaggersQuery {
    reach: Option<UserStreamType>,
}

#[utoipa::path(
    get,
    path = TAG_POST_ROUTE,
    tag = "Global tag Taggers",
    params(
        ("label" = String, Path, description = "Tag name"),
        ("reach" = UserStreamType, Path, description = "Reach type: Follower | Following | Friends")
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
        "GET {TAG_POST_ROUTE} label:{}, reach:{:?}",
        label, query.reach
    );

    let reach = query.reach;

    match Global::get_tag_taggers(label.clone(), reach).await {
        Ok(Some(post)) => Ok(Json(post)),
        Ok(None) => Err(Error::TagsNotFound { reach: label }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(hot_tags_handler, tag_taggers_handler, tags_by_reach_handler),
    components(schemas(HotTags, HotTag, Taggers))
)]
pub struct TagGlobalApiDoc;
