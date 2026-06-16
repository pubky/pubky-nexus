use crate::models::{BoundedLimit, BoundedPagination, BoundedSkip, PostSearchQuery, TagLabel};
use crate::routes::v0::endpoints::{SEARCH_POSTS_BY_CONTENT_ROUTE, SEARCH_POSTS_BY_TAG_ROUTE};
use crate::routes::{Path, Query};
use crate::Result;
use axum::Json;
use nexus_common::models::post::search::{PostsByContentSearch, PostsByTagSearch};
use nexus_common::types::StreamSorting;
use serde::Deserialize;
use tracing::debug;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct SearchPostsQuery {
    pub sorting: Option<StreamSorting>,
    #[serde(flatten)]
    pub pagination: BoundedPagination<10_000, 20, 200>,
    pub start: Option<f64>,
    pub end: Option<f64>,
}

#[utoipa::path(
    get,
    path = SEARCH_POSTS_BY_TAG_ROUTE,
    description = "Search Posts by Tag",
    tag = "Search",
    params(
        ("tag" = TagLabel, Path, description = "Tag name"),
        ("sorting" = Option<StreamSorting>, Query, description = "StreamSorting method"),
        ("start" = Option<f64>, Query, description = "The start of the stream timeframe. Posts with a timestamp greater than this value will be excluded from the results"),
        ("end" = Option<f64>, Query, description = "The end of the stream timeframe. Posts with a timestamp less than this value will be excluded from the results"),
        ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Skip N results (max 10000)"),
        ("limit" = Option<BoundedLimit<20, 200>>, Query, description = "Limit the number of results (1–200, default 20)")
    ),
    responses(
        (status = 200, description = "Search results", body = Vec<PostsByTagSearch>),
        (status = 400, description = "Invalid parameters"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn search_posts_by_tag_handler(
    Path(tag): Path<TagLabel>,
    Query(query): Query<SearchPostsQuery>,
) -> Result<Json<Vec<PostsByTagSearch>>> {
    let sorting = query.sorting;

    debug!(
        "GET {SEARCH_POSTS_BY_TAG_ROUTE} tag:{}, sort_by: {:?}, start: {:?}, end: {:?}, skip: {}, limit: {}",
        tag, sorting, query.start, query.end,
        query.pagination.skip_value(), query.pagination.limit_value()
    );

    let pagination = query.pagination.to_pagination(query.start, query.end);

    match PostsByTagSearch::get_by_label(&tag, sorting, pagination).await? {
        Some(posts_list) => Ok(Json(posts_list)),
        None => Ok(Json(vec![])),
    }
}

#[derive(Deserialize)]
pub struct SearchPostsByContentQuery {
    pub q: PostSearchQuery,
    #[serde(flatten)]
    pub pagination: BoundedPagination<1000, 20, 100>,
}

#[utoipa::path(
    get,
    path = SEARCH_POSTS_BY_CONTENT_ROUTE,
    description = "Full-text search over post content",
    tag = "Search",
    params(
        ("q" = PostSearchQuery, Query, description = "Search query (2–30 characters, up to 4 terms)"),
        ("skip" = Option<BoundedSkip<1000>>, Query, description = "Skip N results (max 1000)"),
        ("limit" = Option<BoundedLimit<20, 100>>, Query, description = "Limit the number of results (1–100, default 20)")
    ),
    responses(
        (status = 200, description = "Search results ordered by relevance score", body = Vec<PostsByContentSearch>),
        (status = 400, description = "Invalid query or limit parameter"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn search_posts_by_content_handler(
    Query(query): Query<SearchPostsByContentQuery>,
) -> Result<Json<Vec<PostsByContentSearch>>> {
    let skip = query.pagination.skip_value();
    let limit = query.pagination.limit_value();

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
    components(schemas(PostsByTagSearch, PostsByContentSearch, PostSearchQuery))
)]
pub struct SearchPostsApiDocs;
