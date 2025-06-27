use crate::routes::v0::endpoints::{
    STREAM_USERS_BY_IDS_ROUTE, STREAM_USERS_ROUTE, STREAM_USERS_USERNAME_SEARCH_ROUTE,
};
use crate::{Error, Result};
use axum::extract::Query;
use axum::Json;
use nexus_common::models::user::{UserStream, UserStreamInput, UserStreamSource};
use nexus_common::types::{Pagination, StreamReach, Timeframe};
use serde::Deserialize;
use tracing::info;
use utoipa::{OpenApi, ToSchema};

#[derive(Deserialize)]
pub struct UserStreamQuery {
    user_id: Option<String>,
    viewer_id: Option<String>,
    skip: Option<usize>,
    limit: Option<usize>,
    source: Option<UserStreamSource>,
    reach: Option<StreamReach>,
    author_id: Option<String>,
    post_id: Option<String>,
    depth: Option<u8>,
    timeframe: Option<Timeframe>,
    preview: Option<bool>,
}

#[utoipa::path(
    get,
    path = STREAM_USERS_ROUTE,
    description = "Stream users",
    tag = "Stream",
    params(
        ("user_id" = Option<String>, Query, description = "User ID to use for streams with source 'following', 'followers', 'friends', 'muted', 'most_followed', 'influencers' and 'recommended'"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("skip" = Option<usize>, Query, description = "Skip N followers"),
        ("limit" = Option<usize>, Query, description = "Retrieve N followers"),
        ("source" = Option<UserStreamSource>, Query, description = "Source of users for the stream."),
        ("reach" = Option<StreamReach>, Query, description = "The target reach of the source. Supported in 'influencers' source."),
        ("timeframe" = Option<Timeframe>, Query, description = "Timeframe for sources supporting a range"),
        ("preview" = Option<bool>, Query, description = "Provide a random selection of size 3 for sources supporting preview. Passing preview ignores skip and limit parameters."),
        ("author_id" = Option<String>, Query, description = "Author id when source is 'post_replies'"),
        ("post_id" = Option<String>, Query, description = "Post id when source is 'post_replies'"),
        ("depth" = Option<usize>, Query, description = "User trusted network depth, user following users distance. Numbers bigger than 4, will be ignored")
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
    info!(
        "GET {STREAM_USERS_ROUTE} viewer_id: {:?} source: {:?}",
        query.viewer_id, query.source
    );

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(5).min(20);
    let source = query.source.unwrap_or(UserStreamSource::Followers);
    let timeframe = query.timeframe.unwrap_or(Timeframe::AllTime);

    if query.user_id.is_none() {
        match source {
            UserStreamSource::Followers => {
                return Err(Error::InvalidInput {
                    message: "user_id query param must be provided for source 'followers'"
                        .to_string(),
                })
            }
            UserStreamSource::Following => {
                return Err(Error::InvalidInput {
                    message: "user_id query param must be provided for source 'following'"
                        .to_string(),
                })
            }
            UserStreamSource::Friends => {
                return Err(Error::InvalidInput {
                    message: "user_id query param must be provided for source 'friends'"
                        .to_string(),
                })
            }
            UserStreamSource::Muted => {
                return Err(Error::InvalidInput {
                    message: "user_id query param must be provided for source 'muted'".to_string(),
                })
            }
            UserStreamSource::Recommended => {
                return Err(Error::InvalidInput {
                    message: "user_id query param must be provided for source 'recommended'"
                        .to_string(),
                })
            }
            UserStreamSource::Influencers => {
                if query.reach.is_some() {
                    return Err(Error::InvalidInput {
                        message:
                            "reach query param must be provided for source 'influencers' with a user_id"
                                .to_string(),
                    });
                }
            }
            UserStreamSource::PostReplies => {
                if query.author_id.is_none() {
                    return Err(Error::InvalidInput {
                        message: "author_id query param must be provided for source 'post_replies'"
                            .to_string(),
                    });
                }
                if query.post_id.is_none() {
                    return Err(Error::InvalidInput {
                        message: "post_id query param must be provided for source 'post_replies'"
                            .to_string(),
                    });
                }
            }
            _ => (),
        }
    }

    match UserStream::get_by_id(
        UserStreamInput {
            user_id: query.user_id.clone(),
            skip: Some(skip),
            limit: Some(limit),
            source: source.clone(),
            reach: query.reach,
            timeframe: Some(timeframe),
            preview: query.preview,
            author_id: query.author_id,
            post_id: query.post_id,
        },
        query.viewer_id,
        query.depth,
    )
    .await
    {
        Ok(Some(stream)) => Ok(Json(stream)),
        Ok(None) => Err(Error::EmptyStream {
            message: format!(
                "No users found for the requested stream: {:?} {:?}",
                source, query.user_id
            ),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(Deserialize)]
pub struct UserStreamSearchQuery {
    username: String,
    viewer_id: Option<String>,
    #[serde(flatten)]
    pagination: Pagination,
}

#[utoipa::path(
    get,
    path = STREAM_USERS_USERNAME_SEARCH_ROUTE,
    tag = "Stream",
    description = "Stream of user from username search result",
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

    let skip = query.pagination.skip.unwrap_or(0);
    let limit = query.pagination.limit.unwrap_or(20);

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
        Ok(None) => Err(Error::EmptyStream {
            message: format!("No users found for the username '{username}'"),
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
    depth: Option<u8>,
}
#[utoipa::path(
    post,
    path = STREAM_USERS_BY_IDS_ROUTE,
    tag = "Stream",
    description = "Stream users by ID. This is a POST request because we're passing a potentially large list of user IDs in the request body.",
    request_body = UserStreamByIdsRequest,
    params(
        ("user_ids" = Vec<String>, Path, description = "Users Pubky ID array"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("depth" = Option<usize>, Query, description = "User trusted network depth, user following users distance. Numbers bigger than 4, will be ignored")
    ),
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

    const MAX_USERS: usize = 100;

    if request.user_ids.len() > MAX_USERS {
        return Err(Error::InvalidInput {
            message: format!("The maximum number of user IDs allowed is {MAX_USERS}"),
        });
    }

    if request.user_ids.is_empty() {
        return Err(Error::InvalidInput {
            message: "The list of user IDs provided is empty".to_string(),
        });
    }

    match UserStream::from_listed_user_ids(
        &request.user_ids,
        request.viewer_id.as_deref(),
        request.depth,
    )
    .await
    {
        Ok(Some(stream)) => Ok(Json(stream)),
        Ok(None) => Err(Error::EmptyStream {
            message: format!(
                "No users found for the requested stream with user ids: {:?}",
                request.user_ids
            ),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        stream_users_handler,
        stream_username_search_handler,
        stream_users_by_ids_handler
    ),
    components(schemas(UserStream, UserStreamSource, UserStreamByIdsRequest))
)]
pub struct StreamUsersApiDocs;
