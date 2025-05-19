use crate::routes::v0::endpoints::SEARCH_TAGS_BY_PREFIX_ROUTE;
use crate::routes::v0::utils::json_array_or_no_content;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use nexus_common::models::tag::global::Taggers;
use nexus_common::models::tag::search::TagSearch;
use nexus_common::types::Pagination;
use nexus_common::types::StreamSorting;
use serde::Deserialize;
use tracing::info;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct SearchTagsQuery {
    pub sorting: Option<StreamSorting>,
    #[serde(flatten)]
    pub pagination: Pagination,
}

#[utoipa::path(
    get,
    path = SEARCH_TAGS_BY_PREFIX_ROUTE,
    description = "Search tags by prefix",
    tag = "Search",
    params(
        ("prefix" = String, Path, description = "Tag name prefix"),
        ("sorting" = Option<StreamSorting>, Query, description = "StreamSorting method"),
        ("start" = Option<usize>, Query, description = "The start of the stream timeframe. Posts with a timestamp greater than this value will be excluded from the results"),
        ("end" = Option<usize>, Query, description = "The end of the stream timeframe. Posts with a timestamp less than this value will be excluded from the results"),
        ("skip" = Option<usize>, Query, description = "Skip N results"),
        ("limit" = Option<usize>, Query, description = "Limit the number of results")
    ),
    responses(
        (status = 200, description = "Search results", body = Vec<TagSearch>),
        (status = 404, description = "No tags with that prefix found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn search_tags_by_prefix_handler(
    Path(prefix): Path<String>,
    Query(query): Query<SearchTagsQuery>,
) -> Result<Json<Vec<String>>> {
    // Extract sorting and pagination fields from the query
    let sorting = query.sorting;
    let mut pagination = query.pagination;

    info!(
        "GET {SEARCH_TAGS_BY_PREFIX_ROUTE} prefix:{}, sort_by: {:?}, start: {:?}, end: {:?}, skip: {:?}, limit: {:?}",
        prefix, sorting, pagination.start, pagination.end, pagination.skip, pagination.limit
    );

    let skip = pagination.skip.unwrap_or(0);
    let limit = pagination.limit.unwrap_or(20);

    pagination.skip = Some(skip);
    pagination.limit = Some(limit);

    match Taggers::get_tags_by_label_prefix(&prefix).await {
        Ok(Some(tags_list)) => json_array_or_no_content(tags_list, "tags"),
        Ok(None) => Err(Error::TagsNotFound {
            reach: String::from("N/A"),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(search_tags_by_prefix_handler), components(schemas(TagSearch)))]
pub struct SearchTagsByPrefixApiDocs;
