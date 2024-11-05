use crate::models::post::PostStreamSorting;
use crate::models::tag::search::TagSearch;
use crate::routes::v0::endpoints::SEARCH_TAGS_ROUTE;
use crate::routes::v0::queries::PaginationQuery;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use log::info;
use serde::Deserialize;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct SearchTagsQuery {
    pub sorting: Option<PostStreamSorting>,
    #[serde(flatten)]
    pub pagination: PaginationQuery,
}

#[utoipa::path(
    get,
    path = SEARCH_TAGS_ROUTE,
    tag = "Search Post by Tags",
    params(
        ("label" = String, Path, description = "Tag name"),
        ("sorting" = Option<PostStreamSorting>, Query, description = "Sorting method"),
        ("start" = Option<usize>, Query, description = "The start of the stream timeframe. Posts with a timestamp greater than this value will be excluded from the results"),
        ("end" = Option<usize>, Query, description = "The end of the stream timeframe. Posts with a timestamp less than this value will be excluded from the results"),
        ("skip" = Option<usize>, Query, description = "Skip N results"),
        ("limit" = Option<usize>, Query, description = "Limit the number of results")
    ),
    responses(
        (status = 200, description = "Search results", body = Vec<TagSearch>),
        (status = 404, description = "No posts with that tag found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn search_post_tags_handler(
    Path(label): Path<String>,
    Query(query): Query<SearchTagsQuery>,
) -> Result<Json<Vec<TagSearch>>> {
    // Extract sorting and pagination fields from the query
    let sorting = query.sorting;
    let mut pagination = query.pagination;

    info!(
        "GET {SEARCH_TAGS_ROUTE} label:{}, sort_by: {:?}, start: {:?}, end: {:?}, skip: {:?}, limit: {:?}",
        label, sorting, pagination.start, pagination.end, pagination.skip, pagination.limit
    );

    let skip = pagination.skip.unwrap_or(0);
    let limit = pagination.limit.unwrap_or(20);

    pagination.skip = Some(skip);
    pagination.limit = Some(limit);

    match TagSearch::get_by_label(&label, sorting, pagination).await {
        Ok(Some(posts_list)) => Ok(Json(posts_list)),
        Ok(None) => Err(Error::PostNotFound {
            author_id: String::from("global"),
            post_id: String::from("N/A"),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(search_post_tags_handler), components(schemas(TagSearch)))]
pub struct SearchTagPostsApiDocs;
