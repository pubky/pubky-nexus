use crate::models::{BoundedLimit, BoundedPagination, BoundedSkip};
use crate::routes::v0::endpoints::SEARCH_TAGS_BY_PREFIX_ROUTE;
use crate::routes::v0::version::{versioned_handlers, ApiVersion};
use crate::routes::Path;
use crate::routes::Query;
use crate::Result;
use axum::Json;
use const_format::concatcp;
use nexus_common::models::tag::search::TagSearch;
use serde::Deserialize;
use tracing::debug;

#[derive(Deserialize)]
pub struct SearchTagsQuery {
    #[serde(flatten)]
    pub pagination: BoundedPagination<10_000, 20, 100>,
}

/// The one body behind `search_tags_by_prefix_handler_v0` and
/// `search_tags_by_prefix_handler_v1`: the search, with the prefix folded as the version folds
/// a label.
pub async fn search_tags_by_prefix<V: ApiVersion>(
    Path(prefix): Path<V::TagLabel>,
    Query(query): Query<SearchTagsQuery>,
) -> Result<Json<Vec<TagSearch>>> {
    let pagination = query.pagination.to_pagination(None, None);

    debug!(
        "GET {SEARCH_TAGS_BY_PREFIX_ROUTE} validated_prefix:{}, skip: {:?}, limit: {:?}",
        prefix, pagination.skip, pagination.limit
    );

    match TagSearch::get_by_label(&prefix, &pagination).await? {
        Some(tags_list) => Ok(Json(tags_list)),
        None => Ok(Json(vec![])),
    }
}

versioned_handlers! {
    search_tags_by_prefix_handler_v0 / search_tags_by_prefix_handler_v1 => search_tags_by_prefix(path: Path<shapes::TagLabel>, query: Query<SearchTagsQuery>) -> Vec<String>;
    #[utoipa::path(
        get,
        path = SEARCH_TAGS_BY_PREFIX_ROUTE,
        operation_id = "search_tags_by_prefix_handler",
        description = "Search tags by prefix",
        tag = "Search",
        params(
            ("prefix" = shapes::TagLabel, Path, description = concatcp!("Tag name prefix", shapes::TAG_PREFIX_RULE)),
            ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Skip N results (max 10000)"),
            ("limit" = Option<BoundedLimit<20, 100>>, Query, description = "Limit the number of results (1–100, default 20)")
        )
    )]
    responses(
        (status = 200, description = "Search results"),
        (status = 400, description = "Invalid parameters"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
}
