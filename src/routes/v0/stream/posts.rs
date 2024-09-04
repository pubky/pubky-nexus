use crate::models::post::{PostStream, PostStreamReach, PostStreamSorting};
use crate::routes::v0::endpoints::{
    STREAM_POSTS_BOOKMARKED_ROUTE, STREAM_POSTS_REACH_ROUTE, STREAM_POSTS_ROUTE,
    STREAM_POSTS_USER_ROUTE,
};
use crate::routes::v0::queries::PostStreamQuery;
use crate::{Error, Result};
use axum::extract::Query;
use axum::Json;
use log::info;
use serde::Deserialize;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = STREAM_POSTS_ROUTE,
    tag = "Stream Posts",
    params(
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("skip" = Option<usize>, Query, description = "Skip N posts"),
        ("limit" = Option<usize>, Query, description = "Retrieve N posts"),
        ("sorting" = Option<PostStreamSorting>, Query, description = "Sorting method")
    ),
    responses(
        (status = 200, description = "Posts stream", body = PostStream),
        (status = 404, description = "Posts not found"),
        (status = 500, description = "Internal server error")
    )
)]

pub async fn stream_global_posts_handler(
    Query(query): Query<PostStreamQuery>,
) -> Result<Json<PostStream>> {
    info!("GET {STREAM_POSTS_ROUTE}");

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(10);
    let sorting = query.sorting.unwrap_or(PostStreamSorting::Timeline);

    match PostStream::get_global_posts(sorting, query.viewer_id, Some(skip), Some(limit)).await {
        Ok(Some(stream)) => Ok(Json(stream)),
        Ok(None) => Err(Error::PostNotFound {
            author_id: String::from("global"),
            post_id: String::from("N/A"),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

use axum::extract::Path;

#[derive(Deserialize)]
pub struct UserPostStreamQuery {
    viewer_id: Option<String>,
    skip: Option<usize>,
    limit: Option<usize>,
}

#[utoipa::path(
    get,
    path = STREAM_POSTS_USER_ROUTE,
    tag = "Stream Posts by User",
    params(
        ("user_id" = String, Path, description = "User ID whose posts to retrieve"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("skip" = Option<usize>, Query, description = "Skip N posts"),
        ("limit" = Option<usize>, Query, description = "Retrieve N posts")
    ),
    responses(
        (status = 200, description = "User's posts stream", body = PostStream),
        (status = 404, description = "Posts not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_user_posts_handler(
    Path(user_id): Path<String>,
    Query(query): Query<UserPostStreamQuery>,
) -> Result<Json<PostStream>> {
    info!("GET {STREAM_POSTS_USER_ROUTE}");

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(10);

    match PostStream::get_user_posts(&user_id, query.viewer_id, Some(skip), Some(limit)).await {
        Ok(Some(stream)) => Ok(Json(stream)),
        Ok(None) => Err(Error::PostNotFound {
            author_id: String::from("global"),
            post_id: String::from("N/A"),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(Deserialize)]
pub struct PostStreamReachQuery {
    viewer_id: String,
    skip: Option<usize>,
    limit: Option<usize>,
    reach: Option<PostStreamReach>,
}

#[utoipa::path(
    get,
    path = STREAM_POSTS_REACH_ROUTE,
    tag = "Stream Posts by Reach",
    params(
        ("viewer_id" = String, Query, description = "Viewer Pubky ID"),
        ("reach" = PostStreamReach, Query, description = "Reach type (Following, Followers, Friends)"),
        ("skip" = Option<usize>, Query, description = "Skip N posts"),
        ("limit" = Option<usize>, Query, description = "Retrieve N posts")
    ),
    responses(
        (status = 200, description = "Posts stream by reach", body = PostStream),
        (status = 404, description = "Posts not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_posts_by_reach_handler(
    Query(query): Query<PostStreamReachQuery>,
) -> Result<Json<PostStream>> {
    info!("GET {STREAM_POSTS_REACH_ROUTE}");

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(10);
    let reach = query.reach.unwrap_or(PostStreamReach::Following);

    match PostStream::get_posts_by_reach(reach, Some(query.viewer_id), Some(skip), Some(limit))
        .await
    {
        Ok(Some(stream)) => Ok(Json(stream)),
        Ok(None) => Err(Error::PostNotFound {
            author_id: String::from("global"),
            post_id: String::from("N/A"),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(Deserialize)]
pub struct BookmarkedPostStreamQuery {
    skip: Option<usize>,
    limit: Option<usize>,
}

#[utoipa::path(
    get,
    path = STREAM_POSTS_BOOKMARKED_ROUTE,
    tag = "Stream Bookmarked Posts",
    params(
        ("user_id" = String, Path, description = "User ID whose bookmarked posts to retrieve"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("skip" = Option<usize>, Query, description = "Skip N posts"),
        ("limit" = Option<usize>, Query, description = "Retrieve N posts")
    ),
    responses(
        (status = 200, description = "Bookmarked posts stream", body = PostStream),
        (status = 404, description = "Posts not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_bookmarked_posts_handler(
    Path(user_id): Path<String>,
    Query(query): Query<BookmarkedPostStreamQuery>,
) -> Result<Json<PostStream>> {
    info!("GET {STREAM_POSTS_BOOKMARKED_ROUTE}");

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(10);

    match PostStream::get_bookmarked_posts(&user_id, Some(skip), Some(limit)).await {
        Ok(Some(stream)) => Ok(Json(stream)),
        Ok(None) => Err(Error::BookmarksNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        stream_global_posts_handler,
        stream_user_posts_handler,
        stream_posts_by_reach_handler,
        stream_bookmarked_posts_handler
    ),
    components(schemas(PostStream, PostStreamSorting, PostStreamReach))
)]
pub struct StreamPostsApiDocs;
