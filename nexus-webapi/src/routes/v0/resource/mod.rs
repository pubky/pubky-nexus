use crate::models::{BoundedLimit, BoundedSkip, PubkyId, ResourceId};
use crate::routes::v0::endpoints::{
    RESOURCE_BY_URI_ROUTE, RESOURCE_TAGGERS_ROUTE, RESOURCE_TAGS_ROUTE,
};
use crate::routes::v0::user::tags::TaggersQuery;
use crate::routes::v0::version::{versioned_handlers, ApiVersion};
use crate::routes::v0::TagsQuery;
use crate::routes::AppState;
use crate::routes::Path;
use crate::routes::Query;
use crate::{Error, Result};
use axum::{Json, Router};
use nexus_common::models::resource::tag::TagResource;
use nexus_common::models::resource::view::ResourceView;
use nexus_common::models::resource::ResourceDetails;
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

impl From<ResourceView> for ResourceTagsResponse {
    fn from(view: ResourceView) -> Self {
        Self {
            resource: view.details,
            tags: view.tags,
        }
    }
}

/// The resource routes for one version: every one serves the version's tags, and the taggers
/// route folds its label the version's way.
pub fn routes<V: ApiVersion>() -> Router<AppState> {
    Router::new()
        .route(RESOURCE_TAGS_ROUTE, V::resource_tags())
        .route(RESOURCE_TAGGERS_ROUTE, V::resource_taggers())
        .route(RESOURCE_BY_URI_ROUTE, V::resource_by_uri())
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

/// The one body behind `resource_tags_handler_v0` and `resource_tags_handler_v1`: the envelope
/// in the version's shape.
pub async fn resource_tags<V: ApiVersion>(
    Path(res_id): Path<ResourceId>,
    Query(query): Query<TagsQuery>,
) -> Result<Json<V::ResourceTagsResponse>> {
    debug!("GET {RESOURCE_TAGS_ROUTE} resource_id:{}", res_id);

    reject_resource_depth(query.depth)?;
    let view = ResourceView::get_by_id(
        &res_id,
        query.skip_tags_as_usize(),
        query.limit_tags_as_usize(),
        query.limit_taggers_as_usize(),
        query.viewer_id.as_deref(),
    )
    .await?
    .ok_or_else(|| Error::resource_not_found(res_id.to_string()))?;

    Ok(Json(V::ResourceTagsResponse::from(view)))
}

versioned_handlers! {
    resource_tags_handler_v0 / resource_tags_handler_v1 => resource_tags(path: Path<ResourceId>, query: Query<TagsQuery>) -> shapes::ResourceTagsResponse;
    #[utoipa::path(
        get,
        path = RESOURCE_TAGS_ROUTE,
        operation_id = "resource_tags_handler",
        description = "Resource tags",
        tag = "Resource",
        params(
            ("resource_id" = ResourceId, Path, description = "Resource ID (32-char hex)"),
            ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID"),
            ("skip_tags" = Option<BoundedSkip<10_000>>, Query, description = "Skip N tags (0–10 000, **default** 0)"),
            ("limit_tags" = Option<BoundedLimit<5, 100>>, Query, description = "Upper limit on the number of tags (1–100, **default** 5)"),
            ("limit_taggers" = Option<BoundedLimit<5, 100>>, Query, description = "Upper limit on the number of taggers per tag (1–100, **default** 5)"),
        )
    )]
    responses(
        (status = 200, description = "Resource tags with metadata"),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "Resource not found"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
}

/// The one body behind `resource_by_uri_handler_v0` and `resource_by_uri_handler_v1`: the
/// envelope in the version's shape.
pub async fn resource_by_uri<V: ApiVersion>(
    Query(query): Query<ResourceByUriQuery>,
) -> Result<Json<V::ResourceTagsResponse>> {
    if query.uri.len() > MAX_URI_LENGTH {
        return Err(Error::invalid_input(format!(
            "URI too long (max {MAX_URI_LENGTH} bytes)"
        )));
    }

    debug!("GET {RESOURCE_BY_URI_ROUTE} uri:{}", query.uri);

    let (normalized, _scheme) = normalize_uri(&query.uri).map_err(Error::invalid_input)?;
    let res_id = resource_id(&normalized);

    reject_resource_depth(query.tags_query.depth)?;
    let view = ResourceView::get_by_id(
        &res_id,
        query.tags_query.skip_tags_as_usize(),
        query.tags_query.limit_tags_as_usize(),
        query.tags_query.limit_taggers_as_usize(),
        query.tags_query.viewer_id.as_deref(),
    )
    .await?
    .ok_or_else(|| Error::resource_not_found(res_id.clone()))?;

    Ok(Json(V::ResourceTagsResponse::from(view)))
}

versioned_handlers! {
    resource_by_uri_handler_v0 / resource_by_uri_handler_v1 => resource_by_uri(query: Query<ResourceByUriQuery>) -> shapes::ResourceTagsResponse;
    #[utoipa::path(
        get,
        path = RESOURCE_BY_URI_ROUTE,
        operation_id = "resource_by_uri_handler",
        description = "Lookup resource tags by raw URI",
        tag = "Resource",
        params(
            ("uri" = String, Query, description = "Raw URI to look up"),
            ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID"),
            ("skip_tags" = Option<BoundedSkip<10_000>>, Query, description = "Skip N tags (0–10 000, **default** 0)"),
            ("limit_tags" = Option<BoundedLimit<5, 100>>, Query, description = "Upper limit on the number of tags (1–100, **default** 5)"),
            ("limit_taggers" = Option<BoundedLimit<5, 100>>, Query, description = "Upper limit on the number of taggers per tag (1–100, **default** 5)"),
        )
    )]
    responses(
        (status = 200, description = "Resource tags with metadata"),
        (status = 404, description = "Resource not found"),
        (status = 400, description = "Invalid URI"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
}

/// The path of `GET /resource/{resource_id}/tags/{label}/taggers`, with `label` folded as
/// version `V` folds it.
#[derive(Deserialize, Debug)]
#[serde(bound(deserialize = ""))]
pub struct ResourceTaggersPath<V: ApiVersion> {
    pub resource_id: ResourceId,
    pub label: V::TagLabel,
}

/// The one body behind `resource_taggers_handler_v0` and `resource_taggers_handler_v1`: the
/// taggers in the version's shape, of the label as the version folds it.
pub async fn resource_taggers<V: ApiVersion>(
    Path(ResourceTaggersPath { resource_id, label }): Path<ResourceTaggersPath<V>>,
    Query(taggers_query): Query<TaggersQuery>,
) -> Result<Json<V::TaggersInfoResponse>> {
    debug!(
        "GET {RESOURCE_TAGGERS_ROUTE} resource_id:{}, label:{}",
        resource_id, label
    );
    reject_resource_depth(taggers_query.tags_query.depth)?;
    let pagination = taggers_query.pagination.to_pagination(None, None);

    let taggers = TagResource::get_taggers(
        &resource_id,
        &label,
        pagination,
        taggers_query.tags_query.viewer_id.as_deref(),
    )
    .await?;
    Ok(Json(V::TaggersInfoResponse::from(taggers)))
}

versioned_handlers! {
    resource_taggers_handler_v0 / resource_taggers_handler_v1 => resource_taggers(path: Path<shapes::ResourceTaggersPath>, query: Query<TaggersQuery>) -> shapes::TaggersInfoResponse;
    #[utoipa::path(
        get,
        path = RESOURCE_TAGGERS_ROUTE,
        operation_id = "resource_taggers_handler",
        description = "Resource specific label taggers",
        tag = "Resource",
        params(
            ("resource_id" = ResourceId, Path, description = "Resource ID (32-char hex)"),
            ("label" = shapes::TagLabel, Path, description = "Tag label"),
            ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID"),
            ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Skip N taggers (0–10 000, **default** 0)"),
            ("limit" = Option<BoundedLimit<40, 100>>, Query, description = "Limit taggers (1–100, default 40)"),
        )
    )]
    responses(
        (status = 200, description = "Resource taggers"),
        (status = 400, description = "Invalid parameters"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
}

/// The schemas the resource routes share; the routes themselves, whose tags, taggers and label
/// are the version's, come with the version's document.
#[derive(OpenApi)]
#[openapi(components(schemas(ResourceDetails, PubkyId, ResourceId)))]
pub struct ResourceApiDoc;
