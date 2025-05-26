use crate::routes::v0::endpoints::SEARCH_POSTS_BY_TAG_ROUTE;
use crate::routes::v0::utils::json_array_or_no_content;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use nexus_common::models::post::search::PostsByTagSearch;
use nexus_common::types::Pagination;
use nexus_common::types::StreamSorting;
use serde::Deserialize;
use tracing::info;
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
        ("tag" = String, Path, description = "Tag name"),
        ("sorting" = Option<StreamSorting>, Query, description = "StreamSorting method"),
        ("start" = Option<usize>, Query, description = "The start of the stream timeframe. Posts with a timestamp greater than this value will be excluded from the results"),
        ("end" = Option<usize>, Query, description = "The end of the stream timeframe. Posts with a timestamp less than this value will be excluded from the results"),
        ("skip" = Option<usize>, Query, description = "Skip N results"),
        ("limit" = Option<usize>, Query, description = "Limit the number of results")
    ),
    responses(
        (status = 200, description = "Search results", body = Vec<PostsByTagSearch>),
        (status = 404, description = "No posts with that tag found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn search_posts_by_tag_handler(
    Path(tag): Path<String>,
    Query(query): Query<SearchPostsQuery>,
) -> Result<Json<Vec<PostsByTagSearch>>> {
    // Extract sorting and pagination fields from the query
    let sorting = query.sorting;
    let mut pagination = query.pagination;

    info!(
        "GET {SEARCH_POSTS_BY_TAG_ROUTE} tag:{}, sort_by: {:?}, start: {:?}, end: {:?}, skip: {:?}, limit: {:?}",
        tag, sorting, pagination.start, pagination.end, pagination.skip, pagination.limit
    );

    let skip = pagination.skip.unwrap_or(0);
    let limit = pagination.limit.unwrap_or(20);

    pagination.skip = Some(skip);
    pagination.limit = Some(limit);

    match PostsByTagSearch::get_by_label(&tag, sorting, pagination).await {
        Ok(Some(posts_list)) => json_array_or_no_content(posts_list, "posts"),
        Ok(None) => Err(Error::PostNotFound {
            author_id: String::from("global"),
            post_id: String::from("N/A"),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(search_posts_by_tag_handler),
    components(schemas(PostsByTagSearch))
)]
pub struct SearchPostsByTagApiDocs;
