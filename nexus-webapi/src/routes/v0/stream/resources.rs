use crate::models::bounded_vec::deserialize_json_array;
use crate::models::{BoundedLimit, BoundedPagination, BoundedSkip, PubkyId, ResourceId, Tags};
use crate::routes::v0::endpoints::{
    STREAM_RESOURCES_BY_IDS_ROUTE, STREAM_RESOURCES_ROUTE, STREAM_RESOURCE_IDS_ROUTE,
};
use crate::routes::Json as RequestJson;
use crate::routes::Query;
use crate::{Error, Result};
use axum::Json;
use nexus_common::db::kv::SortOrder;
use nexus_common::models::resource::stream::{
    ResourceKeyStream, ResourceSorting, ResourceStream, ResourceStreamSource,
};
use nexus_common::models::resource::view::ResourceView;
use nexus_common::universal_tag::normalize::{normalize_uri, resource_id};
use serde::Deserialize;
use tracing::debug;
use utoipa::{OpenApi, ToSchema};

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
        ("sorting" = Option<String>, Query, description = "timeline (the only sorting, and the default)"),
        ("order" = Option<SortOrder>, Query, description = "Ordering of response list. Either 'ascending' or 'descending'. Defaults to descending."),
        ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Pagination skip (max 10000)"),
        ("limit" = Option<BoundedLimit<10, 100>>, Query, description = "Pagination limit (1–100, default 10)"),
        ("start" = Option<f64>, Query, description = "Inclusive score bound the page starts from, following `order`: the latest tag timestamp. Hand back `last_score` with `skip=1` to resume the walk"),
        ("end" = Option<f64>, Query, description = "Inclusive score bound the page stops at, following `order`. Resources scoring past it are excluded"),
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
        ("sorting" = Option<String>, Query, description = "timeline (the only sorting, and the default)"),
        ("order" = Option<SortOrder>, Query, description = "Ordering of response list. Either 'ascending' or 'descending'. Defaults to descending."),
        ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Pagination skip (max 10000)"),
        ("limit" = Option<BoundedLimit<10, 100>>, Query, description = "Pagination limit (1–100, default 10)"),
        ("start" = Option<f64>, Query, description = "Inclusive score bound the page starts from, following `order`: the latest tag timestamp. This route returns no cursor: page it with `skip`/`limit`, or take `last_score` from `/v0/stream/resources/ids`. A view's `details.indexed_at` is the resource's creation time, not the latest tag timestamp the stream sorts on, so passing it here skips resources"),
        ("end" = Option<f64>, Query, description = "Inclusive score bound the page stops at, following `order`. Resources scoring past it are excluded"),
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

/// Supply exactly one list of 1–100 entries. URIs are normalized as `/v0/resource/by-uri` normalizes them.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum ResourceStreamByIdsRequest {
    Ids {
        #[serde(deserialize_with = "deserialize_json_array::<ResourceId, _, 1, 100>")]
        #[schema(min_items = 1, max_items = 100)]
        resource_ids: Vec<ResourceId>,
        viewer_id: Option<PubkyId>,
    },
    Uris {
        /// Each URI must be at most 2048 bytes and have a valid scheme.
        #[serde(deserialize_with = "deserialize_json_array::<String, _, 1, 100>")]
        #[schema(min_items = 1, max_items = 100)]
        uris: Vec<String>,
        viewer_id: Option<PubkyId>,
    },
}

impl ResourceStreamByIdsRequest {
    fn into_ids(self) -> Result<(Vec<String>, Option<PubkyId>)> {
        match self {
            Self::Ids {
                resource_ids,
                viewer_id,
            } => Ok((resource_ids.into_iter().map(|id| id.0).collect(), viewer_id)),
            Self::Uris { uris, viewer_id } => {
                let ids = uris
                    .into_iter()
                    .map(|uri| {
                        if uri.len() > 2048 {
                            return Err(Error::invalid_input("URI exceeds 2048 bytes"));
                        }
                        let (normalized, _) = normalize_uri(&uri).map_err(Error::invalid_input)?;
                        Ok(resource_id(&normalized))
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok((ids, viewer_id))
            }
        }
    }
}

#[utoipa::path(
    post,
    path = STREAM_RESOURCES_BY_IDS_ROUTE,
    tag = "Stream",
    description = "Load 1–100 resources by resource_ids or uris (exactly one list). URIs are normalized and hashed as /v0/resource/by-uri does. Results follow request order, including duplicates; unknown resources are skipped. Each view carries details.id. viewer_id enables viewer-specific tag relationships. No resource is fetched or created by this lookup.",
    request_body = ResourceStreamByIdsRequest,
    responses(
        (status = 200, description = "Resource views in request order; an empty array when nothing matches", body = ResourceStream),
        (status = 400, description = "Invalid body, ID, URI, viewer, or list length"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_resources_by_ids_handler(
    RequestJson(request): RequestJson<ResourceStreamByIdsRequest>,
) -> Result<Json<ResourceStream>> {
    let (ids, viewer_id) = request.into_ids()?;
    let stream = ResourceStream::from_listed_resource_ids(viewer_id.as_deref(), &ids)
        .await?
        .unwrap_or_default();
    Ok(Json(stream))
}

#[derive(OpenApi)]
#[openapi(
    paths(
        stream_resource_ids_handler,
        stream_resources_handler,
        stream_resources_by_ids_handler
    ),
    components(schemas(ResourceKeyStream, SortOrder, PubkyId, ResourceId, Tags))
)]
pub struct StreamResourcesApiDocs;

#[cfg(test)]
mod batch_tests {
    use super::*;
    use serde_json::{json, Value};

    const ID: &str = "19f876713c227a568d939946496a0d30";

    fn validate(value: Value) -> Result<(Vec<String>, Option<PubkyId>)> {
        serde_json::from_value::<ResourceStreamByIdsRequest>(value)
            .map_err(|e| Error::invalid_input(e.to_string()))?
            .into_ids()
    }

    #[test]
    fn accepts_bounded_ids_and_equivalent_uris() {
        assert_eq!(validate(json!({"resource_ids": [ID]})).unwrap().0, [ID]);
        assert_eq!(
            validate(json!({"uris": ["https://Example.com", "https://example.com/"]}))
                .unwrap()
                .0,
            [ID, ID]
        );
        for field in ["resource_ids", "uris"] {
            let entry = if field == "uris" {
                "https://example.com"
            } else {
                ID
            };
            assert_eq!(
                validate(json!({field: vec![entry; 100]})).unwrap().0.len(),
                100
            );
            assert!(validate(json!({field: vec![entry; 101]})).is_err());
            assert!(validate(json!({field: []})).is_err());
        }
    }

    #[test]
    fn rejects_ambiguous_and_invalid_bodies() {
        for body in [
            json!({}),
            json!(null),
            json!({"resource_ids": null}),
            json!({"resource_ids": [ID], "uris": ["https://example.com"]}),
            json!({"resource_ids": [ID], "uris": null}),
            json!({"resource_ids": [ID], "unknown": true}),
            json!({"resource_ids": [ID], "viewer_id": "invalid"}),
            json!({"resource_ids": [ID.to_uppercase()]}),
            json!({"resource_ids": ["short"]}),
            json!({"resource_ids": [42]}),
            json!({"uris": ["not-a-uri"]}),
            json!({"uris": ["1scheme:invalid"]}),
            json!({"uris": [format!("https://example.com/{}", "a".repeat(2048))]}),
            json!({"uris": "https://example.com"}),
        ] {
            assert!(validate(body.clone()).is_err(), "accepted {body}");
        }
    }
}
