use crate::models::user::{UserStream, UserStreamType};
use crate::routes::v0::endpoints::{
    STREAM_USERS_BY_IDS_ROUTE, STREAM_USERS_MOSTFOLLOWED_ROUTE, STREAM_USERS_PIONEERS_ROUTE,
    STREAM_USERS_ROUTE, STREAM_USERS_USERNAME_SEARCH_ROUTE,
};
use crate::{Error, Result};
use axum::extract::Query;
use axum::Json;
use log::info;
use serde::Deserialize;
use utoipa::{OpenApi, ToSchema};

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
        ("user_id" = Option<String>, Query, description = "User Pubky ID"),
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

#[derive(Deserialize)]
pub struct UserStreamSearchQuery {
    username: String,
    viewer_id: Option<String>,
    skip: Option<usize>,
    limit: Option<usize>,
}

#[utoipa::path(
    get,
    path = STREAM_USERS_USERNAME_SEARCH_ROUTE,
    tag = "Stream of Users by Username Search Result",
    params(
        ("username" = String, Query, description = "Username to search for"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("skip" = Option<usize>, Query, description = "Skip N users"),
        ("limit" = Option<usize>, Query, description = "Retrieve N users")
    ),
    responses(
        (status = 200, description = "Username search stream", body = UserStream),
        (status = 400, description = "Bad Request"),
        (status = 404, description = "No users found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_username_search_handler(
    Query(query): Query<UserStreamSearchQuery>,
) -> Result<Json<UserStream>> {
    let username = query.username.trim();
    if username.is_empty() {
        return Err(Error::InvalidInput {
            message: "Username cannot be empty".to_string(),
        });
    }

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(20);

    info!(
        "GET {STREAM_USERS_USERNAME_SEARCH_ROUTE}?username={}",
        username
    );

    match UserStream::get_from_username_search(
        username,
        query.viewer_id.as_deref(),
        Some(skip),
        Some(limit),
    )
    .await
    {
        Ok(Some(stream)) => Ok(Json(stream)),
        Ok(None) => Err(Error::UserNotFound {
            user_id: "No users found for this username".to_string(),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(Deserialize)]
pub struct GlobalUserStreamQuery {
    viewer_id: Option<String>,
    skip: Option<usize>,
    limit: Option<usize>,
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
    Query(query): Query<GlobalUserStreamQuery>,
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

#[utoipa::path(
    get,
    path = STREAM_USERS_PIONEERS_ROUTE,
    tag = "Stream Pioneer Users",
    params(
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("skip" = Option<usize>, Query, description = "Skip N users"),
        ("limit" = Option<usize>, Query, description = "Retrieve N users"),
    ),
    responses(
        (status = 200, description = "Pioneers users stream", body = UserStream),
        (status = 404, description = "No users found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_pioneer_users_handler(
    Query(query): Query<GlobalUserStreamQuery>,
) -> Result<Json<UserStream>> {
    info!("GET {STREAM_USERS_PIONEERS_ROUTE}");

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(15);

    match UserStream::get_by_id(
        "", // No specific user ID is needed for most followed users
        query.viewer_id.as_deref(),
        Some(skip),
        Some(limit),
        UserStreamType::Pioneers,
    )
    .await
    {
        Ok(Some(stream)) => Ok(Json(stream)),
        Ok(None) => Err(Error::UserNotFound {
            user_id: "Pioneers".to_string(),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

// This is a POST request because we're passing a potentially large list of user IDs in the request body,
// which could exceed the URL length limits imposed by some servers and browsers if passed as query parameters.
// Although we're retrieving data, using POST for this type of batch query is a common practice when dealing
// with large request payloads.
#[derive(ToSchema, Deserialize)]
pub struct UserStreamByIdsRequest {
    pub user_ids: Vec<String>,
    pub viewer_id: Option<String>,
}
#[utoipa::path(
    post,
    path = STREAM_USERS_BY_IDS_ROUTE,
    tag = "Stream Users By ID",
    request_body = UserStreamByIdsRequest,
    responses(
        (status = 200, description = "Users stream", body = UserStream),
        (status = 404, description = "Users not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_users_by_ids_handler(
    Json(request): Json<UserStreamByIdsRequest>,
) -> Result<Json<UserStream>> {
    info!(
        "POST {} user_ids: {:?}",
        STREAM_USERS_BY_IDS_ROUTE, request.user_ids
    );

    const MAX_USERS: usize = 1000;

    if request.user_ids.len() > MAX_USERS {
        return Err(Error::InvalidInput {
            message: format!("The maximum number of user IDs allowed is {}", MAX_USERS),
        });
    }

    if request.user_ids.is_empty() {
        return Err(Error::InvalidInput {
            message: "The list of user IDs provided is empty".to_string(),
        });
    }

    match UserStream::from_listed_user_ids(&request.user_ids, request.viewer_id.as_deref()).await {
        Ok(Some(stream)) => Ok(Json(stream)),
        Ok(None) => Err(Error::UserNotFound {
            user_id: "Users not found".to_string(),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        stream_users_handler,
        stream_most_followed_users_handler,
        stream_username_search_handler,
        stream_pioneer_users_handler,
        stream_users_by_ids_handler
    ),
    components(schemas(UserStream, UserStreamType, UserStreamByIdsRequest))
)]
pub struct StreamUsersApiDocs;
