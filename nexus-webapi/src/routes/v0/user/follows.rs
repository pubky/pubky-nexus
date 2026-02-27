use crate::routes::v0::endpoints::{
    USER_FOLLOWERS_ROUTE, USER_FOLLOWING_ROUTE, USER_FRIENDS_ROUTE,
};
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use nexus_common::models::follow::{Followers, Following, Friends, UserFollows};
use nexus_common::types::Pagination;
use tracing::debug;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = USER_FOLLOWERS_ROUTE,
    description = "List user's follower IDs",
    tag = "User",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("skip" = Option<usize>, Query, description = "Skip N followers"),
        ("limit" = Option<usize>, Query, description = "Retrieve N followers")
    ),
    responses(
        (status = 200, description = "User followers list", body = Followers),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_followers_handler(
    Path(user_id): Path<String>,
    Query(query): Query<Pagination>,
) -> Result<Json<Followers>> {
    debug!("GET {USER_FOLLOWERS_ROUTE} user_id:{}", user_id);

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(200);

    match Followers::get_by_id(&user_id, Some(skip), Some(limit)).await? {
        Some(followers) => Ok(Json(followers)),
        None => Err(Error::UserNotFound { user_id }),
    }
}

#[utoipa::path(
    get,
    path = USER_FOLLOWING_ROUTE,
    description = "List user's following IDs",
    tag = "User",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("skip" = Option<usize>, Query, description = "Skip N following"),
        ("limit" = Option<usize>, Query, description = "Retrieve N following")
    ),
    responses(
        (status = 200, description = "User following list", body = Following),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_following_handler(
    Path(user_id): Path<String>,
    Query(query): Query<Pagination>,
) -> Result<Json<Following>> {
    debug!("GET {USER_FOLLOWING_ROUTE} user_id:{}", user_id);

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(200);

    match Following::get_by_id(&user_id, Some(skip), Some(limit)).await? {
        Some(following) => Ok(Json(following)),
        None => Err(Error::UserNotFound { user_id }),
    }
}

#[utoipa::path(
    get,
    path = USER_FRIENDS_ROUTE,
    description = "List user's friend IDs",
    tag = "User",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("skip" = Option<usize>, Query, description = "Skip N friends"),
        ("limit" = Option<usize>, Query, description = "Retrieve N friends")
    ),
    responses(
        (status = 200, description = "User friends list", body = Friends),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_friends_handler(
    Path(user_id): Path<String>,
    Query(query): Query<Pagination>,
) -> Result<Json<Friends>> {
    debug!("GET {USER_FRIENDS_ROUTE} user_id:{}", user_id);

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(200);

    match Friends::get_by_id(&user_id, Some(skip), Some(limit)).await? {
        Some(friends) => Ok(Json(friends)),
        None => Err(Error::UserNotFound { user_id }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(user_followers_handler, user_following_handler, user_friends_handler),
    components(schemas(Followers, Following, Friends))
)]
pub struct UserFollowsApiDoc;
