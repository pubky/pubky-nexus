use crate::routes::v0::endpoints::{STREAM_RESOURCES_ROUTE, STREAM_RESOURCE_IDS_ROUTE};
use crate::Result;
use axum::extract::Query;
use axum::Json;
use nexus_common::db::kv::SortOrder;
use nexus_common::models::resource::stream::{
    ResourceKeyStream, ResourceSorting, ResourceStream, ResourceStreamSource,
};
use nexus_common::models::resource::view::ResourceView;
use nexus_common::types::Pagination;
use serde::Deserialize;
use tracing::debug;
use utoipa::{OpenApi, ToSchema};

const MAX_TAGS: usize = 5;

fn deserialize_comma_separated<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        None => Ok(None),
        Some(s) => {
            let tags: Vec<String> = s
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();
            if tags.len() > MAX_TAGS {
                return Err(serde::de::Error::custom(format!(
                    "Too many tags (max {MAX_TAGS})"
                )));
            }
            if tags.is_empty() {
                Ok(None)
            } else {
                Ok(Some(tags))
            }
        }
    }
}

#[derive(Deserialize, Debug, ToSchema)]
pub struct ResourceStreamQuery {
    pub app: Option<String>,
    #[serde(default, deserialize_with = "deserialize_comma_separated")]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub sorting: ResourceSorting,
    #[serde(default)]
    pub order: Option<SortOrder>,
    #[serde(flatten)]
    pub pagination: Pagination,
    pub viewer_id: Option<String>,
}

#[utoipa::path(
    get,
    path = STREAM_RESOURCE_IDS_ROUTE,
    description = "Stream resource IDs with app/tag filtering",
    tag = "Stream",
    params(
        ("app" = Option<String>, Query, description = "Filter by app namespace (e.g., mapky, eventky)"),
        ("tags" = Option<String>, Query, description = "Comma-separated tag labels (max 5, OR logic)"),
        ("sorting" = Option<String>, Query, description = "timeline or taggers_count"),
        ("skip" = Option<usize>, Query, description = "Pagination skip"),
        ("limit" = Option<usize>, Query, description = "Pagination limit"),
    ),
    responses(
        (status = 200, description = "Resource IDs with cursor", body = ResourceKeyStream),
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

    let keys = ResourceStream::get_resource_keys(
        &source,
        query.pagination,
        order,
        &query.sorting,
        query.tags.as_deref(),
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
        ("tags" = Option<String>, Query, description = "Comma-separated tag labels"),
        ("sorting" = Option<String>, Query, description = "timeline or taggers_count"),
        ("skip" = Option<usize>, Query, description = "Pagination skip"),
        ("limit" = Option<usize>, Query, description = "Pagination limit"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID for relationship checks"),
    ),
    responses(
        (status = 200, description = "Resource stream", body = Vec<ResourceView>),
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

    let keys = ResourceStream::get_resource_keys(
        &source,
        query.pagination,
        order,
        &query.sorting,
        query.tags.as_deref(),
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
    components(schemas(ResourceKeyStream, ResourceStreamQuery))
)]
pub struct StreamResourcesApiDocs;
