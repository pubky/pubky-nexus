use crate::models::user::{Followers, Following};
use crate::routes::v0::endpoints::{USER_FOLLOWERS_ROUTE, USER_FOLLOWING_ROUTE};
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use log::info;
use serde::Deserialize;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct FollowsQuery {
    skip: Option<usize>,
    limit: Option<usize>,
}

#[utoipa::path(
    get,
    path = USER_FOLLOWERS_ROUTE,
    tag = "User Followers List",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("skip" = Option<usize>, Query, description = "Skip N followers"),
        ("limit" = Option<usize>, Query, description = "Retrive N followers")
    ),
    responses(
        (status = 200, description = "User followers list", body = Follows),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_followers_handler(
    Path(user_id): Path<String>,
    Query(query): Query<FollowsQuery>,
) -> Result<Json<Followers>> {
    info!("GET {USER_FOLLOWERS_ROUTE} user_id:{}", user_id);

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(200);

    match Followers::get_by_id(&user_id, Some(skip), Some(limit)).await {
        Ok(Some(followers)) => Ok(Json(followers)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[utoipa::path(
    get,
    path = USER_FOLLOWING_ROUTE,
    tag = "User Following List",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("skip" = Option<usize>, Query, description = "Skip N following"),
        ("limit" = Option<usize>, Query, description = "Retrive N following")
    ),
    responses(
        (status = 200, description = "User following list", body = Follows),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_following_handler(
    Path(user_id): Path<String>,
    Query(query): Query<FollowsQuery>,
) -> Result<Json<Following>> {
    info!("GET {USER_FOLLOWING_ROUTE} user_id:{}", user_id);

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(200);

    match Following::get_by_id(&user_id, Some(skip), Some(limit)).await {
        Ok(Some(following)) => Ok(Json(following)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(user_followers_handler, user_following_handler),
    components(schemas(Followers, Following))
)]
pub struct UserFollowsApiDoc;
