use crate::models::{BoundedLimit, BoundedPagination, BoundedSkip, TagLabel};
use crate::routes::v0::endpoints::SEARCH_TAGS_BY_PREFIX_ROUTE;
use crate::routes::Path;
use crate::routes::Query;
use crate::Result;
use axum::Json;
use nexus_common::models::tag::search::TagSearch;
use serde::Deserialize;
use tracing::debug;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct SearchTagsQuery {
    #[serde(flatten)]
    pub pagination: BoundedPagination<10_000, 20, 100>,
}

#[utoipa::path(
    get,
    path = SEARCH_TAGS_BY_PREFIX_ROUTE,
    description = "Search tags by prefix",
    tag = "Search",
    params(
        ("prefix" = TagLabel, Path, description = "Tag name prefix"),
        ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Skip N results (max 10000)"),
        ("limit" = Option<BoundedLimit<20, 100>>, Query, description = "Limit the number of results (1–100, default 20)")
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
    let pagination = query.pagination.to_pagination(None, None);

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
