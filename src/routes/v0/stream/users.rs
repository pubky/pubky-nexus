use crate::models::user::{UserStream, UserStreamType};
use crate::routes::v0::endpoints::{STREAM_USERS_MOSTFOLLOWED_ROUTE, STREAM_USERS_ROUTE};
use crate::{Error, Result};
use axum::extract::Query;
use axum::Json;
use log::info;
use serde::Deserialize;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct UserStreamQuery {
    user_id: Option<String>,
    viewer_id: Option<String>,
    skip: Option<usize>,
    limit: Option<usize>,
    stream_type: Option<UserStreamType>,
}

#[utoipa::path(
    get,
    path = STREAM_USERS_ROUTE,
    tag = "Stream Users",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("skip" = Option<usize>, Query, description = "Skip N followers"),
        ("limit" = Option<usize>, Query, description = "Retrieve N followers"),
        ("stream_type" = Option<UserStreamType>, Query, description = "Stream Type")
    ),
    responses(
        (status = 200, description = "Users stream", body = UserStream),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    )
)]

pub async fn stream_users_handler(
    Query(query): Query<UserStreamQuery>,
) -> Result<Json<UserStream>> {
    let user_id = match query.user_id {
        Some(user_id) => user_id,
        None => {
            return Err(Error::UserNotFound {
                user_id: "user_id query param not provided".to_string(),
            })
        }
    };

    info!("GET {STREAM_USERS_ROUTE} user_id:{}", user_id);

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(20);
    let stream_type = query.stream_type.unwrap_or(UserStreamType::Followers);

    match UserStream::get_by_id(
        &user_id,
        query.viewer_id.as_deref(),
        Some(skip),
        Some(limit),
        stream_type,
    )
    .await
    {
        Ok(Some(stream)) => Ok(Json(stream)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[utoipa::path(
    get,
    path = STREAM_USERS_MOSTFOLLOWED_ROUTE,
    tag = "Stream Most Followed Users",
    params(
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("skip" = Option<usize>, Query, description = "Skip N users"),
        ("limit" = Option<usize>, Query, description = "Retrieve N users"),
    ),
    responses(
        (status = 200, description = "Most followed users stream", body = UserStream),
        (status = 404, description = "No users found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_most_followed_users_handler(
    Query(query): Query<UserStreamQuery>,
) -> Result<Json<UserStream>> {
    info!("GET {STREAM_USERS_MOSTFOLLOWED_ROUTE}");

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(15);

    match UserStream::get_by_id(
        "", // No specific user ID is needed for most followed users
        query.viewer_id.as_deref(),
        Some(skip),
        Some(limit),
        UserStreamType::MostFollowed,
    )
    .await
    {
        Ok(Some(stream)) => Ok(Json(stream)),
        Ok(None) => Err(Error::UserNotFound {
            user_id: "Most Followed".to_string(),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(stream_users_handler, stream_most_followed_users_handler),
    components(schemas(UserStream, UserStreamType))
)]
pub struct StreamUsersApiDocs;
