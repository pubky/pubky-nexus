use crate::models::{PostSearchQuery, SearchLimit, TagLabel, SEARCH_LIMIT_DEFAULT};
use crate::routes::v0::endpoints::{SEARCH_POSTS_BY_CONTENT_ROUTE, SEARCH_POSTS_BY_TAG_ROUTE};
use crate::routes::{Path, Query};
use crate::Result;
use axum::Json;
use nexus_common::models::post::search::{PostsByContentSearch, PostsByTagSearch};
use nexus_common::types::Pagination;
use nexus_common::types::StreamSorting;
use serde::Deserialize;
use tracing::debug;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct SearchPostsQuery {
    pub sorting: Option<StreamSorting>,
    #[serde(flatten)]
    pub pagination: Pagination,
}

#[utoipa::path(
    get,
    path = SEARCH_POSTS_BY_TAG_ROUTE,
    description = "Search Posts by Tag",
    tag = "Search",
    params(
        ("tag" = TagLabel, Path, description = "Tag name"),
        ("sorting" = Option<StreamSorting>, Query, description = "StreamSorting method"),
        ("start" = Option<usize>, Query, description = "The start of the stream timeframe. Posts with a timestamp greater than this value will be excluded from the results"),
        ("end" = Option<usize>, Query, description = "The end of the stream timeframe. Posts with a timestamp less than this value will be excluded from the results"),
        ("skip" = Option<usize>, Query, description = "Skip N results"),
        ("limit" = Option<usize>, Query, description = "Limit the number of results")
    ),
    responses(
        (status = 200, description = "Search results", body = Vec<PostsByTagSearch>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn search_posts_by_tag_handler(
    Path(tag): Path<TagLabel>,
    Query(query): Query<SearchPostsQuery>,
) -> Result<Json<Vec<PostsByTagSearch>>> {
    // Extract sorting and pagination fields from the query
    let sorting = query.sorting;
    let mut pagination = query.pagination;

    debug!(
        "GET {SEARCH_POSTS_BY_TAG_ROUTE} tag:{}, sort_by: {:?}, start: {:?}, end: {:?}, skip: {:?}, limit: {:?}",
        tag, sorting, pagination.start, pagination.end, pagination.skip, pagination.limit
    );

    let skip = pagination.skip.unwrap_or(0);
    let limit = pagination.limit.unwrap_or(20);

    pagination.skip = Some(skip);
    pagination.limit = Some(limit);

    match PostsByTagSearch::get_by_label(&tag, sorting, pagination).await? {
        Some(posts_list) => Ok(Json(posts_list)),
        None => Ok(Json(vec![])),
    }
}

#[derive(Deserialize)]
pub struct SearchPostsByContentQuery {
    pub q: PostSearchQuery,
    pub skip: Option<usize>,
    pub limit: Option<SearchLimit<100>>,
}

#[utoipa::path(
    get,
    path = SEARCH_POSTS_BY_CONTENT_ROUTE,
    description = "Full-text search over post content",
    tag = "Search",
    params(
        ("q" = PostSearchQuery, Query, description = "Search query (2–200 characters)"),
        ("skip" = Option<usize>, Query, description = "Skip N results"),
        ("limit" = Option<SearchLimit<100>>, Query, description = "Limit the number of results")
    ),
    responses(
        (status = 200, description = "Search results ordered by relevance score", body = Vec<PostsByContentSearch>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn search_posts_by_content_handler(
    Query(query): Query<SearchPostsByContentQuery>,
) -> Result<Json<Vec<PostsByContentSearch>>> {
    let skip = query.skip.unwrap_or(0);
    let limit = query
        .limit
        .as_ref()
        .map_or(SEARCH_LIMIT_DEFAULT, SearchLimit::value);

    debug!(
        "GET {SEARCH_POSTS_BY_CONTENT_ROUTE} q:{}, skip:{skip}, limit:{limit}",
        query.q
    );

    let results = PostsByContentSearch::search(query.q.as_str(), skip, limit).await?;
    Ok(Json(results))
}

#[derive(OpenApi)]
#[openapi(
    paths(search_posts_by_tag_handler, search_posts_by_content_handler),
    components(schemas(PostsByTagSearch, PostsByContentSearch, SearchLimit<100>, PostSearchQuery))
)]
pub struct SearchPostsApiDocs;
