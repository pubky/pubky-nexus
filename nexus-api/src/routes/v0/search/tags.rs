use crate::routes::v0::endpoints::SEARCH_TAGS_BY_PREFIX_ROUTE;
use crate::routes::v0::utils::json_array_or_no_content;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use nexus_common::models::post::search::PostsByTagSearch;
use nexus_common::models::tag::global::Taggers;
use nexus_common::types::Pagination;
use serde::Deserialize;
use tracing::info;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct SearchTagsQuery {
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
        ("skip" = Option<usize>, Query, description = "Skip N results"),
        ("limit" = Option<usize>, Query, description = "Limit the number of results")
    ),
    responses(
        (status = 200, description = "Search results", body = Vec<String>),
        (status = 404, description = "No tags with that prefix found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn search_tags_by_prefix_handler(
    Path(prefix): Path<String>,
    Query(query): Query<SearchTagsQuery>,
) -> Result<Json<Vec<String>>> {
    let mut pagination = query.pagination;
    pagination.skip.get_or_insert_default();
    pagination.limit.get_or_insert(20);

    info!(
        "GET {SEARCH_TAGS_BY_PREFIX_ROUTE} prefix:{}, skip: {:?}, limit: {:?}",
        prefix, pagination.skip, pagination.limit
    );

    match Taggers::get_tags_by_label_prefix(&prefix, &pagination).await {
        Ok(Some(tags_list)) => json_array_or_no_content(tags_list, "tags"),
        Ok(None) => Err(Error::TagsNotFound {
            reach: String::from("N/A"),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(search_tags_by_prefix_handler),
    components(schemas(PostsByTagSearch))
)]
pub struct SearchTagsByPrefixApiDocs;
