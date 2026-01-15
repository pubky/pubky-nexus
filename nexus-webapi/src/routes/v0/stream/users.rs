use crate::routes::v0::endpoints::{
    STREAM_USERS_BY_IDS_ROUTE, STREAM_USERS_ROUTE, STREAM_USERS_USERNAME_SEARCH_ROUTE,
    STREAM_USER_IDS_ROUTE,
};
use crate::{Error, Result};
use axum::extract::Query;
use axum::Json;
use nexus_common::models::user::{UserIdStream, UserStream, UserStreamInput, UserStreamSource};
use nexus_common::types::{Pagination, StreamReach, Timeframe};
use serde::Deserialize;
use tracing::debug;
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
    tag = "Stream",
    params(
        ("source" = Option<UserStreamSource>, Query, description = "Source of users for streams (followers, following, friends, muted, most_followed, influencers, recommended, post_replies)"),
        ("user_id" = Option<String>, Query, description = "User ID to use for streams with source 'following', 'followers', 'friends', 'muted', 'influencers' and 'recommended'"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("author_id" = Option<String>, Query, description = "Author ID when source is 'post_replies'"),
        ("post_id" = Option<String>, Query, description = "Post ID when source is 'post_replies'"),
        ("reach" = Option<StreamReach>, Query, description = "The target reach of the source. Supported in 'influencers' source."),
        ("timeframe" = Option<Timeframe>, Query, description = "Timeframe for sources supporting a range"),
        ("preview" = Option<bool>, Query, description = "Provide a random selection of size 3 for sources supporting preview. Passing preview ignores skip and limit parameters."),
        ("depth" = Option<u8>, Query, description = "User trusted network depth, user following users distance. Numbers bigger than 3 will be ignored"),
        ("skip" = Option<usize>, Query, description = "Skip N users"),
        ("limit" = Option<usize>, Query, description = "Retrieve N users")
    ),
    responses(
        (status = 200, description = "Users stream", body = UserStream),
        (status = 500, description = "Internal server error")
    ),
    description = r#"Stream Users: Retrieve a stream of users.

The `source` parameter determines the type of stream. Depending on the `source`, certain parameters are required:
- *following*, *followers*, *friends*, *muted*, *recommended*: Requires **user_id**.
- *influencers*: When **user_id** is provided with a **timeframe** (not 'all_time'), **reach** determines the network scope for finding influencers.The **reach** parameter can be: 'followers', 'following', 'friends', 'wot' (defaults to depth 3), or 'wot_1', 'wot_2', 'wot_3'. Defaults to 'wot_3' if not specified. If **user_id** is not provided, returns global influencers.
- *post_replies*: Requires **author_id** and **post_id** to filter replies to a specific post.
- *most_followed*: Does not require **user_id**.

Ensure that you provide the necessary parameters based on the selected `source`. If the required parameter is not provided, an error will be returned."#
)]
pub async fn stream_users_handler(
    Query(query): Query<UserStreamQuery>,
) -> Result<Json<UserStream>> {
    debug!(
        "GET {STREAM_USERS_ROUTE} viewer_id: {:?} source: {:?}",
        query.viewer_id, query.source
    );

    let (input, viewer_id, depth) = build_user_stream_input(query)?;

    match UserStream::get_by_id(input, viewer_id, depth).await? {
        Some(stream) => Ok(Json(stream)),
        None => Ok(Json(UserStream::default())),
    }
}

#[utoipa::path(
    get,
    path = STREAM_USER_IDS_ROUTE,
    tag = "Stream",
    params(
        ("source" = Option<UserStreamSource>, Query, description = "Source of users for streams (followers, following, friends, muted, most_followed, influencers, recommended, post_replies)"),
        ("user_id" = Option<String>, Query, description = "User ID to use for streams with source 'following', 'followers', 'friends', 'muted', 'influencers' and 'recommended'"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("author_id" = Option<String>, Query, description = "Author ID when source is 'post_replies'"),
        ("post_id" = Option<String>, Query, description = "Post ID when source is 'post_replies'"),
        ("reach" = Option<StreamReach>, Query, description = "The target reach of the source. Supported in 'influencers' source."),
        ("timeframe" = Option<Timeframe>, Query, description = "Timeframe for sources supporting a range"),
        ("preview" = Option<bool>, Query, description = "Provide a random selection of size 3 for sources supporting preview. Passing preview ignores skip and limit parameters."),
        ("depth" = Option<u8>, Query, description = "User trusted network depth, user following users distance. Numbers bigger than 3 will be ignored"),
        ("skip" = Option<usize>, Query, description = "Skip N users"),
        ("limit" = Option<usize>, Query, description = "Retrieve N users")
    ),
    responses(
        (status = 200, description = "User IDs stream", body = UserIdStream),
        (status = 500, description = "Internal server error")
    ),
    description = r#"Stream User IDs: Retrieve a stream of user identifiers.

The `source` parameter determines the type of stream. Depending on the `source`, certain parameters are required:
- *following*, *followers*, *friends*, *muted*, *recommended*: Requires **user_id**.
- *influencers*: When **user_id** is provided with a **timeframe** (not 'all_time'), **reach** determines the network scope for finding influencers.The **reach** parameter can be: 'followers', 'following', 'friends', 'wot' (defaults to depth 3), or 'wot_1', 'wot_2', 'wot_3'. Defaults to 'wot_3' if not specified. If **user_id** is not provided, returns global influencers.
- *post_replies*: Requires **author_id** and **post_id** to filter replies to a specific post.
- *most_followed*: Does not require **user_id**.

Ensure that you provide the necessary parameters based on the selected `source`. If the required parameter is not provided, an error will be returned."#
)]
pub async fn stream_user_ids_handler(
    Query(query): Query<UserStreamQuery>,
) -> Result<Json<UserIdStream>> {
    debug!(
        "GET {STREAM_USER_IDS_ROUTE} viewer_id: {:?} source: {:?}",
        query.viewer_id, query.source
    );

    let (input, _, _) = build_user_stream_input(query)?;

    match UserStream::get_user_list_from_source(input).await? {
        Some(user_ids) => Ok(Json(UserIdStream::new(user_ids))),
        None => Ok(Json(UserIdStream::default())),
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
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_username_search_handler(
    Query(query): Query<UserStreamSearchQuery>,
) -> Result<Json<UserStream>> {
    let username = query.username.trim();
    if username.is_empty() {
        return Err(Error::invalid_input("Username cannot be empty"));
    }

    let skip = query.pagination.skip.unwrap_or(0);
    let limit = query.pagination.limit.unwrap_or(20);

    debug!("GET {STREAM_USERS_USERNAME_SEARCH_ROUTE}?username={username}");

    match UserStream::get_from_username_search(
        username,
        query.viewer_id.as_deref(),
        Some(skip),
        Some(limit),
    )
    .await?
    {
        Some(stream) => Ok(Json(stream)),
        None => Ok(Json(UserStream::default())),
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
        ("user_ids" = Vec<String>, Path, description = "User Pubky ID array"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("depth" = Option<u8>, Query, description = "User trusted network depth, user following users distance. Numbers bigger than 4 will be ignored")
    ),
    responses(
        (status = 200, description = "Users stream", body = UserStream),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_users_by_ids_handler(
    Json(request): Json<UserStreamByIdsRequest>,
) -> Result<Json<UserStream>> {
    debug!(
        "POST {} user_ids: {:?}",
        STREAM_USERS_BY_IDS_ROUTE, request.user_ids
    );

    const MAX_USERS: usize = 100;

    if request.user_ids.len() > MAX_USERS {
        let err_msg = format!("The maximum number of user IDs allowed is {MAX_USERS}");
        return Err(Error::invalid_input(&err_msg));
    }

    if request.user_ids.is_empty() {
        let err_msg = "The list of user IDs provided is empty";
        return Err(Error::invalid_input(err_msg));
    }

    match UserStream::from_listed_user_ids(
        &request.user_ids,
        request.viewer_id.as_deref(),
        request.depth,
    )
    .await?
    {
        Some(stream) => Ok(Json(stream)),
        None => Ok(Json(UserStream::default())),
    }
}

fn build_user_stream_input(
    query: UserStreamQuery,
) -> Result<(UserStreamInput, Option<String>, Option<u8>)> {
    let UserStreamQuery {
        user_id,
        viewer_id,
        skip,
        limit,
        source,
        reach,
        author_id,
        post_id,
        depth,
        timeframe,
        preview,
    } = query;

    let source = source.unwrap_or(UserStreamSource::Followers);
    let skip = skip.unwrap_or(0);
    let limit = limit.unwrap_or(5).min(20);
    let timeframe = timeframe.unwrap_or(Timeframe::AllTime);

    if user_id.is_none() {
        match source {
            UserStreamSource::Followers
            | UserStreamSource::Following
            | UserStreamSource::Friends
            | UserStreamSource::Muted
            | UserStreamSource::Recommended => {
                return Err(Error::invalid_input(&format!(
                    "user_id query param must be provided for source '{}'",
                    source_name(&source)
                )));
            }
            UserStreamSource::Influencers if reach.is_some() => {
                return Err(Error::invalid_input(
                    "user_id query param must be provided for source 'influencers' when reach is specified",
                ));
            }
            UserStreamSource::PostReplies => {
                if author_id.is_none() {
                    return Err(Error::invalid_input(
                        "author_id query param must be provided for source 'post_replies'",
                    ));
                }
                if post_id.is_none() {
                    return Err(Error::invalid_input(
                        "post_id query param must be provided for source 'post_replies'",
                    ));
                }
            }
            _ => (),
        }
    }

    let input = UserStreamInput {
        user_id,
        skip: Some(skip),
        limit: Some(limit),
        source: source.clone(),
        reach,
        timeframe: Some(timeframe),
        preview,
        author_id,
        post_id,
    };

    Ok((input, viewer_id, depth))
}

/// Returns the snake_case name of the source for error messages.
fn source_name(source: &UserStreamSource) -> &'static str {
    match source {
        UserStreamSource::Followers => "followers",
        UserStreamSource::Following => "following",
        UserStreamSource::Friends => "friends",
        UserStreamSource::Muted => "muted",
        UserStreamSource::MostFollowed => "most_followed",
        UserStreamSource::Influencers => "influencers",
        UserStreamSource::Recommended => "recommended",
        UserStreamSource::PostReplies => "post_replies",
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        stream_users_handler,
        stream_user_ids_handler,
        stream_username_search_handler,
        stream_users_by_ids_handler
    ),
    components(schemas(
        UserIdStream,
        UserStream,
        UserStreamSource,
        UserStreamByIdsRequest,
        StreamReach,
        Timeframe
    ))
)]
pub struct StreamUsersApiDocs;
