use crate::routes::v0::endpoints::SEARCH_TAGS_BY_PREFIX_ROUTE;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use nexus_common::models::post::search::PostsByTagSearch;
use nexus_common::models::tag::search::TagSearch;
use nexus_common::types::Pagination;
use pubky_app_specs::traits::Validatable;
use pubky_app_specs::PubkyAppTag;
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
        ("prefix" = String, Path, description = "Tag name prefix"),
        ("skip" = Option<usize>, Query, description = "Skip N results"),
        ("limit" = Option<usize>, Query, description = "Limit the number of results")
    ),
    responses(
        (status = 200, description = "Search results", body = Vec<String>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn search_tags_by_prefix_handler(
    Path(prefix): Path<String>,
    Query(query): Query<SearchTagsQuery>,
) -> Result<Json<Vec<TagSearch>>> {
    let validated_prefix = sanitize_validate(&prefix)?;

    let mut pagination = query.pagination;
    pagination.skip.get_or_insert_default();
    pagination.limit.get_or_insert(20);

    debug!(
        "GET {SEARCH_TAGS_BY_PREFIX_ROUTE} validated_prefix:{}, skip: {:?}, limit: {:?}",
        validated_prefix, pagination.skip, pagination.limit
    );

    match TagSearch::get_by_label(&validated_prefix, &pagination).await? {
        Some(tags_list) => Ok(Json(tags_list)),
        None => Ok(Json(vec![])),
    }
}

fn sanitize_validate(tag_prefix: &str) -> Result<String> {
    // Use a throwaway URI to build the tag instance, as we only need it for validation
    let temp_tag = PubkyAppTag::new(
        "pubky://user_pubky_id/pub/pubky.app/profile.json".into(),
        tag_prefix.into(),
    );

    temp_tag
        .validate(None)
        .map_err(|e| Error::invalid_input(&e.to_string()))?;

    let sanitized = temp_tag.label;
    Ok(sanitized)
}

#[derive(OpenApi)]
#[openapi(
    paths(search_tags_by_prefix_handler),
    components(schemas(PostsByTagSearch))
)]
pub struct SearchTagsByPrefixApiDocs;
