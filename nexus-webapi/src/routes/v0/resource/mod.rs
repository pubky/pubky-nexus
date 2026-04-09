use crate::routes::v0::endpoints::{
    RESOURCE_BY_URI_ROUTE, RESOURCE_TAGGERS_ROUTE, RESOURCE_TAGS_ROUTE,
};
use crate::routes::v0::user::tags::TaggersQuery;
use crate::routes::v0::{TaggersInfoResponse, TagsQuery};
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::routing::get;
use axum::{Json, Router};
use nexus_common::db::fetch_row_from_graph;
use nexus_common::models::resource::tag::TagResource;
use nexus_common::models::resource::{normalize_uri, resource_id, ResourceDetails};
use nexus_common::models::tag::traits::{TagCollection, TaggersCollection};
use nexus_common::models::tag::TagDetails;
use serde::{Deserialize, Serialize};
use tracing::debug;
use utoipa::{OpenApi, ToSchema};

use crate::routes::AppState;

/// Max length for URI used in raw URI lookup
const MAX_URI_LENGTH: usize = 2048;

/// Response envelope for resource tag endpoints, matching spec Section 9.1.
#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct ResourceTagsResponse {
    pub resource: ResourceDetails,
    pub tags: Vec<TagDetails>,
}

/// Validates that a resource_id is a 32-char lowercase hex string.
fn validate_resource_id(id: &str) -> std::result::Result<(), Error> {
    if id.len() != 32
        || !id
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err(Error::InvalidInput {
            message: format!("resource_id must be 32-char lowercase hex, got: {id}"),
        });
    }
    Ok(())
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(RESOURCE_TAGS_ROUTE, get(resource_tags_handler))
        .route(RESOURCE_TAGGERS_ROUTE, get(resource_taggers_handler))
        .route(RESOURCE_BY_URI_ROUTE, get(resource_by_uri_handler))
}

#[derive(Deserialize, Debug)]
pub struct ResourceByUriQuery {
    pub uri: String,
    #[serde(flatten)]
    pub tags_query: TagsQuery,
}

#[utoipa::path(
    get,
    path = RESOURCE_TAGS_ROUTE,
    description = "Resource tags",
    tag = "Resource",
    params(
        ("resource_id" = String, Path, description = "Resource ID (32-char hex)"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("skip_tags" = Option<usize>, Query, description = "Skip N tags"),
        ("limit_tags" = Option<usize>, Query, description = "Limit tags"),
        ("limit_taggers" = Option<usize>, Query, description = "Limit taggers per tag"),
    ),
    responses(
        (status = 404, description = "Resource not found"),
        (status = 200, description = "Resource tags with metadata", body = ResourceTagsResponse),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn resource_tags_handler(
    Path(res_id): Path<String>,
    Query(query): Query<TagsQuery>,
) -> Result<Json<ResourceTagsResponse>> {
    validate_resource_id(&res_id)?;
    debug!("GET {RESOURCE_TAGS_ROUTE} resource_id:{}", res_id);

    let tags = TagResource::get_by_id(
        &res_id,
        None,
        query.skip_tags,
        query.limit_tags,
        query.limit_taggers,
        query.viewer_id.as_deref(),
        query.depth,
    )
    .await?
    .ok_or_else(|| Error::ResourceNotFound {
        resource_id: res_id.clone(),
    })?;

    let resource = load_resource_details(&res_id).await?;

    Ok(Json(ResourceTagsResponse { resource, tags }))
}

#[utoipa::path(
    get,
    path = RESOURCE_BY_URI_ROUTE,
    description = "Lookup resource tags by raw URI",
    tag = "Resource",
    params(
        ("uri" = String, Query, description = "Raw URI to look up"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("skip_tags" = Option<usize>, Query, description = "Skip N tags"),
        ("limit_tags" = Option<usize>, Query, description = "Limit tags"),
        ("limit_taggers" = Option<usize>, Query, description = "Limit taggers per tag"),
    ),
    responses(
        (status = 404, description = "Resource not found"),
        (status = 200, description = "Resource tags with metadata", body = ResourceTagsResponse),
        (status = 400, description = "Invalid URI"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn resource_by_uri_handler(
    Query(query): Query<ResourceByUriQuery>,
) -> Result<Json<ResourceTagsResponse>> {
    if query.uri.len() > MAX_URI_LENGTH {
        return Err(Error::invalid_input(&format!(
            "URI too long (max {MAX_URI_LENGTH} bytes)"
        )));
    }

    debug!("GET {RESOURCE_BY_URI_ROUTE} uri:{}", query.uri);

    let (normalized, _scheme) =
        normalize_uri(&query.uri).map_err(|e| Error::InvalidInput { message: e })?;
    let res_id = resource_id(&normalized);

    let tags = TagResource::get_by_id(
        &res_id,
        None,
        query.tags_query.skip_tags,
        query.tags_query.limit_tags,
        query.tags_query.limit_taggers,
        query.tags_query.viewer_id.as_deref(),
        query.tags_query.depth,
    )
    .await?
    .ok_or_else(|| Error::ResourceNotFound {
        resource_id: res_id.clone(),
    })?;

    let resource = load_resource_details(&res_id).await?;

    Ok(Json(ResourceTagsResponse { resource, tags }))
}

#[utoipa::path(
    get,
    path = RESOURCE_TAGGERS_ROUTE,
    description = "Resource specific label taggers",
    tag = "Resource",
    params(
        ("resource_id" = String, Path, description = "Resource ID (32-char hex)"),
        ("label" = String, Path, description = "Tag label"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("skip" = Option<usize>, Query, description = "Skip N taggers"),
        ("limit" = Option<usize>, Query, description = "Limit taggers"),
    ),
    responses(
        (status = 200, description = "Resource taggers", body = TaggersInfoResponse),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn resource_taggers_handler(
    Path((resource_id, label)): Path<(String, String)>,
    Query(taggers_query): Query<TaggersQuery>,
) -> Result<Json<TaggersInfoResponse>> {
    validate_resource_id(&resource_id)?;
    debug!(
        "GET {RESOURCE_TAGGERS_ROUTE} resource_id:{}, label:{}",
        resource_id, label
    );
    let taggers = TagResource::get_tagger_by_id(
        &resource_id,
        None,
        &label,
        taggers_query.pagination,
        taggers_query.tags_query.viewer_id.as_deref(),
        taggers_query.tags_query.depth,
    )
    .await?;
    Ok(Json(TaggersInfoResponse::from(taggers)))
}

/// Load Resource node details from Neo4j.
async fn load_resource_details(res_id: &str) -> Result<ResourceDetails> {
    let query = nexus_common::db::queries::get::get_resource_by_id(res_id);
    let maybe_row = fetch_row_from_graph(query)
        .await
        .map_err(|e| Error::InternalServerError {
            source: Box::new(e),
        })?;

    match maybe_row {
        Some(row) => Ok(ResourceDetails {
            id: row.get("id").unwrap_or_default(),
            uri: row.get("uri").unwrap_or_default(),
            scheme: row.get("scheme").unwrap_or_default(),
            indexed_at: row.get("indexed_at").unwrap_or(0),
        }),
        None => Err(Error::ResourceNotFound {
            resource_id: res_id.to_string(),
        }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        resource_tags_handler,
        resource_by_uri_handler,
        resource_taggers_handler
    ),
    components(schemas(ResourceTagsResponse, ResourceDetails, TagDetails, TaggersInfoResponse))
)]
pub struct ResourceApiDoc;
