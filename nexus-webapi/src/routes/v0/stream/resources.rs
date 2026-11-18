use crate::models::{BoundedLimit, BoundedPagination, BoundedSkip, PubkyId, TagLabels};
use crate::routes::v0::endpoints::{STREAM_RESOURCES_ROUTE, STREAM_RESOURCE_IDS_ROUTE};
use crate::routes::v0::version::{versioned_handlers, ApiVersion};
use crate::routes::Query;
use crate::{Error, Result};
use axum::Json;
use const_format::concatcp;
use nexus_common::db::kv::SortOrder;
use nexus_common::models::resource::stream::{ResourceKeyStream, ResourceSorting, ResourceStream};
use serde::Deserialize;
use tracing::debug;
use utoipa::OpenApi;

/// The `app` filter Nexus no longer stores. A request that still sends it gets 400 rather
/// than a silently unfiltered stream, so a client notices the break. It leaves the query in a
/// later release.
const APP_REMOVED: &str = "app is no longer supported; filter by tags";

/// The query of `GET /stream/resources` and `GET /stream/resources/ids`, with the labels of
/// `tags` folded by version `V`'s rule. Every other parameter is the same on both.
#[derive(Deserialize, Debug)]
#[serde(bound(deserialize = ""))]
pub struct ResourceStreamQuery<V: ApiVersion> {
    /// Removed filter: any value, the empty string included, is rejected with 400.
    pub app: Option<String>,
    pub tags: Option<V::Tags>,
    #[serde(default)]
    pub sorting: ResourceSorting,
    #[serde(default)]
    pub order: Option<SortOrder>,
    #[serde(flatten)]
    pub pagination: BoundedPagination<10_000, 10, 100>,
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub viewer_id: Option<PubkyId>,
}

impl<V: ApiVersion> ResourceStreamQuery<V> {
    /// Rejects a request that still sends the removed `app` filter.
    fn reject_app(&self) -> Result<()> {
        match self.app {
            Some(_) => Err(Error::invalid_input(APP_REMOVED)),
            None => Ok(()),
        }
    }
}

/// The one body behind `stream_resource_ids_handler_v0` and `stream_resource_ids_handler_v1`:
/// the keys of the page, filtered by the labels as the version folds them.
pub async fn stream_resource_ids<V: ApiVersion>(
    Query(query): Query<ResourceStreamQuery<V>>,
) -> Result<Json<ResourceKeyStream>> {
    debug!(
        "GET {STREAM_RESOURCE_IDS_ROUTE} tags:{:?}, sorting:{:?}",
        query.tags, query.sorting
    );

    query.reject_app()?;

    let order = query.order.unwrap_or(SortOrder::Descending);
    let tags = query.tags.as_ref().map(TagLabels::to_string_vec);
    let pagination = query.pagination.to_pagination(query.start, query.end);

    let keys =
        ResourceStream::get_resource_keys(pagination, order, &query.sorting, tags.as_deref())
            .await?;

    Ok(Json(keys))
}

versioned_handlers! {
    stream_resource_ids_handler_v0 / stream_resource_ids_handler_v1 => stream_resource_ids(query: Query<shapes::ResourceStreamQuery>) -> ResourceKeyStream;
    #[utoipa::path(
        get,
        path = STREAM_RESOURCE_IDS_ROUTE,
        operation_id = "stream_resource_ids_handler",
        description = "Stream resource IDs with tag filtering",
        tag = "Stream",
        params(
            ("app" = Option<String>, Query, description = "Removed. A request that sends it gets 400; filter by `tags` instead"),
            ("tags" = Option<shapes::Tags>, Query, description = "Comma-separated tag labels (max 5, OR logic)"),
            ("sorting" = Option<String>, Query, description = "timeline (the only sorting, and the default)"),
            ("order" = Option<SortOrder>, Query, description = "Ordering of response list. Either 'ascending' or 'descending'. Defaults to descending."),
            ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Pagination skip (max 10000)"),
            ("limit" = Option<BoundedLimit<10, 100>>, Query, description = "Pagination limit (1–100, default 10)"),
            ("start" = Option<f64>, Query, description = "Inclusive score bound the page starts from, following `order`: the latest tag timestamp. Hand back `last_score` with `skip=1` to resume the walk"),
            ("end" = Option<f64>, Query, description = "Inclusive score bound the page stops at, following `order`. Resources scoring past it are excluded"),
        )
    )]
    responses(
        (status = 200, description = "Resource IDs with cursor"),
        (status = 400, description = "Invalid parameters, or the removed `app` filter"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
}

/// The one body behind `stream_resources_handler_v0` and `stream_resources_handler_v1`: the
/// views in the version's shape, filtered by the labels as the version folds them.
pub async fn stream_resources<V: ApiVersion>(
    Query(query): Query<ResourceStreamQuery<V>>,
) -> Result<Json<Vec<V::ResourceView>>> {
    debug!(
        "GET {STREAM_RESOURCES_ROUTE} tags:{:?}, sorting:{:?}",
        query.tags, query.sorting
    );

    query.reject_app()?;

    let order = query.order.unwrap_or(SortOrder::Descending);
    let tags = query.tags.as_ref().map(TagLabels::to_string_vec);
    let pagination = query.pagination.to_pagination(query.start, query.end);

    let keys =
        ResourceStream::get_resource_keys(pagination, order, &query.sorting, tags.as_deref())
            .await?;

    if keys.is_empty() {
        return Ok(Json(vec![]));
    }

    let stream =
        ResourceStream::from_listed_resource_ids(query.viewer_id.as_deref(), &keys.resource_ids)
            .await?;

    let views = stream
        .map(|s| s.0)
        .unwrap_or_default()
        .into_iter()
        .map(V::ResourceView::from)
        .collect();
    Ok(Json(views))
}

versioned_handlers! {
    stream_resources_handler_v0 / stream_resources_handler_v1 => stream_resources(query: Query<shapes::ResourceStreamQuery>) -> Vec<shapes::ResourceView>;
    #[utoipa::path(
        get,
        path = STREAM_RESOURCES_ROUTE,
        operation_id = "stream_resources_handler",
        description = "Stream resources with tag filtering, returns full ResourceView objects",
        tag = "Stream",
        params(
            ("app" = Option<String>, Query, description = "Removed. A request that sends it gets 400; filter by `tags` instead"),
            ("tags" = Option<shapes::Tags>, Query, description = "Comma-separated tag labels (max 5, OR logic)"),
            ("sorting" = Option<String>, Query, description = "timeline (the only sorting, and the default)"),
            ("order" = Option<SortOrder>, Query, description = "Ordering of response list. Either 'ascending' or 'descending'. Defaults to descending."),
            ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Pagination skip (max 10000)"),
            ("limit" = Option<BoundedLimit<10, 100>>, Query, description = "Pagination limit (1–100, default 10)"),
            ("start" = Option<f64>, Query, description = concatcp!("Inclusive score bound the page starts from, following `order`: the latest tag timestamp. This route returns no cursor: page it with `skip`/`limit`, or take `last_score` from `", shapes::PREFIX, "/stream/resources/ids`. A view's `details.indexed_at` is the resource's creation time, not the latest tag timestamp the stream sorts on, so passing it here skips resources")),
            ("end" = Option<f64>, Query, description = "Inclusive score bound the page stops at, following `order`. Resources scoring past it are excluded"),
            ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID for relationship checks"),
        )
    )]
    responses(
        (status = 200, description = "Resource stream"),
        (status = 400, description = "Invalid parameters, or the removed `app` filter"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
}

/// The schemas both resource stream routes share; the routes themselves, whose label list is
/// the version's and whose views are on the resource stream, come with the version's document.
#[derive(OpenApi)]
#[openapi(components(schemas(ResourceKeyStream, SortOrder, PubkyId)))]
pub struct StreamResourcesApiDocs;
