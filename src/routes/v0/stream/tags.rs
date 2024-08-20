use crate::models::tag::stream::HotTag;
use crate::models::user::UserStreamType;
use crate::routes::v0::endpoints::{STREAM_TAGS_GLOBAL_ROUTE, STREAM_TAGS_REACH_ROUTE};
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use log::{ info, error };
use serde::Deserialize;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct HotTagsQuery {
    skip: Option<usize>,
    limit: Option<usize>
}

#[utoipa::path(
    get,
    path = STREAM_TAGS_GLOBAL_ROUTE,
    params(
        ("skip" = Option<usize>, Query, description = "Skip N tags"),
        ("limit" = Option<usize>, Query, description = "Retrieve N tag")
    ),
    tag = "Stream of hot tags",
    responses(
        (status = 200, description = "Retrieve hot tags stream", body = Vec<StreamTag>),
        (status = 404, description = "Hot tags not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_hot_tags_handler(
    Query(query): Query<HotTagsQuery>,
) -> Result<Json<Vec<HotTag>>> {
    info!("GET {STREAM_TAGS_GLOBAL_ROUTE}");

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(40);
    
    match HotTag::get_global_tags_stream(Some(skip), Some(limit)).await {
        Ok(Some(hot_tags)) => Ok(Json(hot_tags)),
        Ok(None) => Err(Error::TagsNotFound { reach: String::from("GLOBAL") }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(Deserialize)]
pub struct StreamTagsReachInput {
    user_id: String,
    reach: Option<UserStreamType>,
}

#[utoipa::path(
    get,
    path = STREAM_TAGS_REACH_ROUTE,
    tag = "Stream of tags by reach",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("reach" = UserStreamType, Path, description = "Reach type: Follower | Following | Friends")
    ),
    responses(
        (status = 200, description = "Retrieve tags by reach cluster", body = StreamTags),
        (status = 404, description = "Hot tags not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_tags_by_reach_handler(
    Path(path): Path<StreamTagsReachInput>
) -> Result<Json<Vec<HotTag>>> {
    info!("GET {STREAM_TAGS_REACH_ROUTE}");

    let reach = path.reach.unwrap_or(UserStreamType::Following);
    let user_id = path.user_id;
    
    match HotTag::get_stream_tags_by_reach(user_id, reach).await {
        Ok(Some(hot_tags)) => Ok(Json(hot_tags)),
        Ok(None) => Err(Error::TagsNotFound { reach: String::from("REACH") }),
        Err(source) => {
            error!("Internal Server ERROR: {:?}", source);
            Err(Error::InternalServerError { source })
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        stream_hot_tags_handler,
        stream_tags_by_reach_handler
    ),
    components(schemas(HotTag))
)]
pub struct StreamTagsApiDocs;
