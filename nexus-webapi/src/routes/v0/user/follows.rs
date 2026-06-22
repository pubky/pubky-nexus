use crate::models::{BoundedLimit, BoundedPagination, BoundedSkip, PubkyId};
use crate::routes::v0::endpoints::{
    USER_FOLLOWERS_ROUTE, USER_FOLLOWING_ROUTE, USER_FRIENDS_ROUTE,
};
use crate::routes::Path;
use crate::routes::Query;
use crate::{Error, Result};
use axum::Json;
use nexus_common::models::follow::{Followers, Following, Friends, UserFollows};
use serde::Deserialize;
use tracing::debug;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct FollowsQuery {
    #[serde(flatten)]
    pub pagination: BoundedPagination<10_000, 50, 200>,
}

#[utoipa::path(
    get,
    path = USER_FOLLOWERS_ROUTE,
    description = "List user's follower IDs",
    tag = "User",
    params(
        ("user_id" = PubkyId, Path, description = "User Pubky ID"),
        ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Skip N followers (max 10000)"),
        ("limit" = Option<BoundedLimit<50, 200>>, Query, description = "Retrieve N followers (1–200, default 50)")
    ),
    responses(
        (status = 200, description = "User followers list", body = Followers),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_followers_handler(
    Path(user_id): Path<PubkyId>,
    Query(query): Query<FollowsQuery>,
) -> Result<Json<Followers>> {
    debug!("GET {USER_FOLLOWERS_ROUTE} user_id:{}", user_id);

    let pagination = query.pagination.to_pagination(None, None);

    match Followers::get_by_id(&user_id, pagination.skip, pagination.limit).await? {
        Some(followers) => Ok(Json(followers)),
        None => Err(Error::user_not_found(user_id)),
    }
}

#[utoipa::path(
    get,
    path = USER_FOLLOWING_ROUTE,
    description = "List user's following IDs",
    tag = "User",
    params(
        ("user_id" = PubkyId, Path, description = "User Pubky ID"),
        ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Skip N following (max 10000)"),
        ("limit" = Option<BoundedLimit<50, 200>>, Query, description = "Retrieve N following (1–200, default 50)")
    ),
    responses(
        (status = 200, description = "User following list", body = Following),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_following_handler(
    Path(user_id): Path<PubkyId>,
    Query(query): Query<FollowsQuery>,
) -> Result<Json<Following>> {
    debug!("GET {USER_FOLLOWING_ROUTE} user_id:{}", user_id);

    let pagination = query.pagination.to_pagination(None, None);

    match Following::get_by_id(&user_id, pagination.skip, pagination.limit).await? {
        Some(following) => Ok(Json(following)),
        None => Err(Error::user_not_found(user_id)),
    }
}

#[utoipa::path(
    get,
    path = USER_FRIENDS_ROUTE,
    description = "List user's friend IDs",
    tag = "User",
    params(
        ("user_id" = PubkyId, Path, description = "User Pubky ID"),
        ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Skip N friends (max 10000)"),
        ("limit" = Option<BoundedLimit<50, 200>>, Query, description = "Retrieve N friends (1–200, default 50)")
    ),
    responses(
        (status = 200, description = "User friends list", body = Friends),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_friends_handler(
    Path(user_id): Path<PubkyId>,
    Query(query): Query<FollowsQuery>,
) -> Result<Json<Friends>> {
    debug!("GET {USER_FRIENDS_ROUTE} user_id:{}", user_id);

    let pagination = query.pagination.to_pagination(None, None);

    match Friends::get_by_id(&user_id, pagination.skip, pagination.limit).await? {
        Some(friends) => Ok(Json(friends)),
        None => Err(Error::user_not_found(user_id)),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(user_followers_handler, user_following_handler, user_friends_handler),
    components(schemas(Followers, Following, Friends, PubkyId))
)]
pub struct UserFollowsApiDoc;
