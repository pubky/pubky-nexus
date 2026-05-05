use crate::models::TagLabel;
use crate::routes::v0::endpoints::SEARCH_TAGS_BY_PREFIX_ROUTE;
use crate::routes::Path;
use crate::Result;
use axum::extract::Query;
use axum::Json;
use nexus_common::models::tag::search::TagSearch;
use nexus_common::types::Pagination;
use serde::Deserialize;
use tracing::debug;
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
        ("prefix" = TagLabel, Path, description = "Tag name prefix"),
        ("skip" = Option<usize>, Query, description = "Skip N results"),
        ("limit" = Option<usize>, Query, description = "Limit the number of results")
    ),
    responses(
        (status = 200, description = "Search results", body = Vec<String>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn search_tags_by_prefix_handler(
    Path(prefix): Path<TagLabel>,
    Query(query): Query<SearchTagsQuery>,
) -> Result<Json<Vec<TagSearch>>> {
    let mut pagination = query.pagination;
    pagination.skip.get_or_insert_default();
    pagination.limit.get_or_insert(20);

    debug!(
        "GET {SEARCH_TAGS_BY_PREFIX_ROUTE} validated_prefix:{}, skip: {:?}, limit: {:?}",
        prefix, pagination.skip, pagination.limit
    );

    match TagSearch::get_by_label(prefix.as_str(), &pagination).await? {
        Some(tags_list) => Ok(Json(tags_list)),
        None => Ok(Json(vec![])),
    }
}

#[derive(OpenApi)]
#[openapi(paths(search_tags_by_prefix_handler), components(schemas(TagLabel)))]
pub struct SearchTagsByPrefixApiDocs;
