use crate::models::{BoundedLimit, BoundedPagination, BoundedSkip, PubkyId, Tags};
use crate::routes::v0::endpoints::{STREAM_RESOURCES_ROUTE, STREAM_RESOURCE_IDS_ROUTE};
use crate::routes::Query;
use crate::Result;
use axum::Json;
use nexus_common::db::kv::SortOrder;
use nexus_common::models::resource::stream::{
    ResourceKeyStream, ResourceSorting, ResourceStream, ResourceStreamSource,
};
use nexus_common::models::resource::view::ResourceView;
use serde::Deserialize;
use tracing::debug;
use utoipa::OpenApi;

#[derive(Deserialize, Debug)]
pub struct ResourceStreamQuery {
    pub app: Option<String>,
    pub tags: Option<Tags>,
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

#[utoipa::path(
    get,
    path = STREAM_RESOURCE_IDS_ROUTE,
    description = "Stream resource IDs with app/tag filtering",
    tag = "Stream",
    params(
        ("app" = Option<String>, Query, description = "Filter by app namespace (e.g., mapky, eventky)"),
        ("tags" = Option<Tags>, Query, description = "Comma-separated tag labels (max 5, OR logic)"),
        ("sorting" = Option<String>, Query, description = "timeline or taggers_count"),
        ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Pagination skip (max 10000)"),
        ("limit" = Option<BoundedLimit<10, 100>>, Query, description = "Pagination limit (1–100, default 10)"),
    ),
    responses(
        (status = 200, description = "Resource IDs with cursor", body = ResourceKeyStream),
        (status = 400, description = "Invalid parameters"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_resource_ids_handler(
    Query(query): Query<ResourceStreamQuery>,
) -> Result<Json<ResourceKeyStream>> {
    debug!(
        "GET {STREAM_RESOURCE_IDS_ROUTE} app:{:?}, tags:{:?}, sorting:{:?}",
        query.app, query.tags, query.sorting
    );

    let source = match &query.app {
        Some(app) => ResourceStreamSource::App { app: app.clone() },
        None => ResourceStreamSource::All,
    };

    let order = query.order.unwrap_or(SortOrder::Descending);
    let tags = query.tags.as_ref().map(Tags::to_string_vec);
    let pagination = query.pagination.to_pagination(query.start, query.end);

    let keys = ResourceStream::get_resource_keys(
        &source,
        pagination,
        order,
        &query.sorting,
        tags.as_deref(),
    )
    .await?;

    Ok(Json(keys))
}

#[utoipa::path(
    get,
    path = STREAM_RESOURCES_ROUTE,
    description = "Stream resources with app/tag filtering, returns full ResourceView objects",
    tag = "Stream",
    params(
        ("app" = Option<String>, Query, description = "Filter by app namespace"),
        ("tags" = Option<Tags>, Query, description = "Comma-separated tag labels (max 5, OR logic)"),
        ("sorting" = Option<String>, Query, description = "timeline or taggers_count"),
        ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Pagination skip (max 10000)"),
        ("limit" = Option<BoundedLimit<10, 100>>, Query, description = "Pagination limit (1–100, default 10)"),
        ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID for relationship checks"),
    ),
    responses(
        (status = 200, description = "Resource stream", body = Vec<ResourceView>),
        (status = 400, description = "Invalid parameters"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_resources_handler(
    Query(query): Query<ResourceStreamQuery>,
) -> Result<Json<Vec<ResourceView>>> {
    debug!(
        "GET {STREAM_RESOURCES_ROUTE} app:{:?}, tags:{:?}, sorting:{:?}",
        query.app, query.tags, query.sorting
    );

    let source = match &query.app {
        Some(app) => ResourceStreamSource::App { app: app.clone() },
        None => ResourceStreamSource::All,
    };

    let order = query.order.unwrap_or(SortOrder::Descending);
    let tags = query.tags.as_ref().map(Tags::to_string_vec);
    let pagination = query.pagination.to_pagination(query.start, query.end);

    let keys = ResourceStream::get_resource_keys(
        &source,
        pagination,
        order,
        &query.sorting,
        tags.as_deref(),
    )
    .await?;

    if keys.is_empty() {
        return Ok(Json(vec![]));
    }

    let stream =
        ResourceStream::from_listed_resource_ids(query.viewer_id.as_deref(), &keys.resource_ids)
            .await?;

    Ok(Json(stream.map(|s| s.0).unwrap_or_default()))
}

#[derive(OpenApi)]
#[openapi(
    paths(stream_resource_ids_handler, stream_resources_handler),
    components(schemas(ResourceKeyStream, PubkyId, Tags))
)]
pub struct StreamResourcesApiDocs;
