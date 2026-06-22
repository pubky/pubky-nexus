use crate::models::{BoundedLimit, BoundedSkip, PubkyId, ResourceId, TagLabel};
use crate::routes::v0::endpoints::{
    RESOURCE_BY_URI_ROUTE, RESOURCE_TAGGERS_ROUTE, RESOURCE_TAGS_ROUTE,
};
use crate::routes::v0::user::tags::TaggersQuery;
use crate::routes::v0::{TaggersInfoResponse, TagsQuery};
use crate::routes::AppState;
use crate::routes::Path;
use crate::routes::Query;
use crate::{Error, Result};
use axum::routing::get;
use axum::{Json, Router};
use nexus_common::models::resource::tag::TagResource;
use nexus_common::models::resource::ResourceDetails;
use nexus_common::models::tag::traits::{TagCollection, TaggersCollection};
use nexus_common::models::tag::TagDetails;
use nexus_common::universal_tag::normalize::{normalize_uri, resource_id};
use serde::{Deserialize, Serialize};
use tracing::debug;
use utoipa::{OpenApi, ToSchema};

/// Max length for URI used in raw URI lookup
const MAX_URI_LENGTH: usize = 2048;

/// Response envelope for resource tag endpoints, matching spec Section 9.1.
#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct ResourceTagsResponse {
    pub resource: ResourceDetails,
    pub tags: Vec<TagDetails>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(RESOURCE_TAGS_ROUTE, get(resource_tags_handler))
        .route(RESOURCE_TAGGERS_ROUTE, get(resource_taggers_handler))
        .route(RESOURCE_BY_URI_ROUTE, get(resource_by_uri_handler))
}

/// Resource tags have no Web-of-Trust variant: the WoT tag query matches a
/// `User` node, so feeding it a resource id silently returns nothing and 404s
/// an existing resource. Reject `depth` outright instead of pretending to
/// support it (ponytail: 400 over a broken feature; add real resource-WoT here
/// if it's ever needed).
fn reject_resource_depth(depth: Option<u8>) -> Result<()> {
    match depth {
        Some(_) => Err(Error::invalid_input(
            "`depth` is not supported for resource tags",
        )),
        None => Ok(()),
    }
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
        ("resource_id" = ResourceId, Path, description = "Resource ID (32-char hex)"),
        ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID"),
        ("skip_tags" = Option<BoundedSkip<10_000>>, Query, description = "Skip N tags (0–10 000, **default** 0)"),
        ("limit_tags" = Option<BoundedLimit<5, 100>>, Query, description = "Upper limit on the number of tags (1–100, **default** 5)"),
        ("limit_taggers" = Option<BoundedLimit<5, 100>>, Query, description = "Upper limit on the number of taggers per tag (1–100, **default** 5)"),
    ),
    responses(
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "Resource not found"),
        (status = 200, description = "Resource tags with metadata", body = ResourceTagsResponse),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn resource_tags_handler(
    Path(res_id): Path<ResourceId>,
    Query(query): Query<TagsQuery>,
) -> Result<Json<ResourceTagsResponse>> {
    debug!("GET {RESOURCE_TAGS_ROUTE} resource_id:{}", res_id);

    reject_resource_depth(query.depth)?;
    let tags = TagResource::get_by_id(
        &res_id,
        None,
        query.skip_tags_as_usize(),
        query.limit_tags_as_usize(),
        query.limit_taggers_as_usize(),
        query.viewer_id.as_deref(),
        None,
    )
    .await?
    .ok_or_else(|| Error::resource_not_found(res_id.to_string()))?;

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
        ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID"),
        ("skip_tags" = Option<BoundedSkip<10_000>>, Query, description = "Skip N tags (0–10 000, **default** 0)"),
        ("limit_tags" = Option<BoundedLimit<5, 100>>, Query, description = "Upper limit on the number of tags (1–100, **default** 5)"),
        ("limit_taggers" = Option<BoundedLimit<5, 100>>, Query, description = "Upper limit on the number of taggers per tag (1–100, **default** 5)"),
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
        return Err(Error::invalid_input(format!(
            "URI too long (max {MAX_URI_LENGTH} bytes)"
        )));
    }

    debug!("GET {RESOURCE_BY_URI_ROUTE} uri:{}", query.uri);

    let (normalized, _scheme) = normalize_uri(&query.uri).map_err(Error::invalid_input)?;
    let res_id = resource_id(&normalized);

    reject_resource_depth(query.tags_query.depth)?;
    let tags = TagResource::get_by_id(
        &res_id,
        None,
        query.tags_query.skip_tags_as_usize(),
        query.tags_query.limit_tags_as_usize(),
        query.tags_query.limit_taggers_as_usize(),
        query.tags_query.viewer_id.as_deref(),
        None,
    )
    .await?
    .ok_or_else(|| Error::resource_not_found(res_id.clone()))?;

    let resource = load_resource_details(&res_id).await?;

    Ok(Json(ResourceTagsResponse { resource, tags }))
}

#[derive(Deserialize, Debug)]
pub struct ResourceTaggersPath {
    pub resource_id: ResourceId,
    pub label: TagLabel,
}

#[utoipa::path(
    get,
    path = RESOURCE_TAGGERS_ROUTE,
    description = "Resource specific label taggers",
    tag = "Resource",
    params(
        ("resource_id" = ResourceId, Path, description = "Resource ID (32-char hex)"),
        ("label" = TagLabel, Path, description = "Tag label"),
        ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID"),
        ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Skip N taggers (0–10 000, **default** 0)"),
        ("limit" = Option<BoundedLimit<40, 100>>, Query, description = "Limit taggers (1–100, default 40)"),
    ),
    responses(
        (status = 400, description = "Invalid parameters"),
        (status = 200, description = "Resource taggers", body = TaggersInfoResponse),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn resource_taggers_handler(
    Path(ResourceTaggersPath { resource_id, label }): Path<ResourceTaggersPath>,
    Query(taggers_query): Query<TaggersQuery>,
) -> Result<Json<TaggersInfoResponse>> {
    debug!(
        "GET {RESOURCE_TAGGERS_ROUTE} resource_id:{}, label:{}",
        resource_id, label
    );
    reject_resource_depth(taggers_query.tags_query.depth)?;
    let pagination = taggers_query.pagination.to_pagination(None, None);

    let taggers = TagResource::get_tagger_by_id(
        &resource_id,
        None,
        &label,
        pagination,
        taggers_query.tags_query.viewer_id.as_deref(),
        None,
    )
    .await?;
    Ok(Json(TaggersInfoResponse::from(taggers)))
}

/// Load Resource node details from Neo4j.
async fn load_resource_details(res_id: &str) -> Result<ResourceDetails> {
    ResourceDetails::get_by_id(res_id)
        .await?
        .ok_or_else(|| Error::resource_not_found(res_id))
}

#[derive(OpenApi)]
#[openapi(
    paths(
        resource_tags_handler,
        resource_by_uri_handler,
        resource_taggers_handler
    ),
    components(schemas(
        ResourceTagsResponse,
        ResourceDetails,
        TagDetails,
        TaggersInfoResponse,
        PubkyId,
        ResourceId,
        TagLabel
    ))
)]
pub struct ResourceApiDoc;
