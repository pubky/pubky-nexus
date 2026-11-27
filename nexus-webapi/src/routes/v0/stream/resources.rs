use crate::models::bounded_vec::deserialize_json_array;
use crate::models::{
    v1, BoundedLimit, BoundedPagination, BoundedSkip, PubkyId, ResourceId, TagLabels,
};
use crate::routes::v0::endpoints::{
    STREAM_RESOURCES_BY_IDS_ROUTE, STREAM_RESOURCES_ROUTE, STREAM_RESOURCE_IDS_ROUTE,
};
use crate::routes::v0::resource::MAX_URI_LENGTH;
use crate::routes::v0::version::{versioned_handlers, ApiVersion, V1};
use crate::routes::Json as RequestJson;
use crate::routes::Query;
use crate::{Error, Result};
use axum::Json;
use const_format::concatcp;
use nexus_common::db::kv::SortOrder;
use nexus_common::models::resource::stream::{
    ResourceKeyStream, ResourceSorting, ResourceStream, TagMatch,
};
use nexus_common::universal_tag::normalize::{normalize_uri, resource_id};
use serde::Deserialize;
use std::collections::HashSet;
use tracing::debug;
use utoipa::openapi::schema::{
    AdditionalProperties, ArrayBuilder, ObjectBuilder, OneOfBuilder, Schema, Type,
};
use utoipa::openapi::{Ref, RefOr};
use utoipa::{OpenApi, PartialSchema, ToSchema};

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
    /// `any` or `all` on `/v1`, nothing on `/v0`, which is frozen and always matches `any`.
    #[serde(default, rename = "match")]
    pub tags_match: V::TagsMatch,
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

    /// The labels and how they match, once the request is checked: no removed `app`, and
    /// `match=all` only with 1 to [`TagMatch::MAX_ALL_LABELS`] distinct labels.
    fn filter(&self) -> Result<(Option<Vec<String>>, TagMatch)> {
        self.reject_app()?;
        let tags = self.tags.as_ref().map(TagLabels::to_string_vec);
        let tag_match: TagMatch = self.tags_match.into();
        if tag_match == TagMatch::All {
            let Some(labels) = &tags else {
                return Err(Error::invalid_input("match=all needs tags"));
            };
            let distinct: HashSet<&String> = labels.iter().collect();
            if distinct.len() > TagMatch::MAX_ALL_LABELS {
                return Err(Error::invalid_input(format!(
                    "match=all takes at most {} distinct tags",
                    TagMatch::MAX_ALL_LABELS
                )));
            }
        }
        Ok((tags, tag_match))
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

    let (tags, tag_match) = query.filter()?;

    let order = query.order.unwrap_or(SortOrder::Descending);
    let pagination = query.pagination.to_pagination(query.start, query.end);

    let keys = ResourceStream::get_resource_keys(
        pagination,
        order,
        &query.sorting,
        tags.as_deref(),
        tag_match,
    )
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
            ("tags" = Option<shapes::Tags>, Query, description = concatcp!("Comma-separated tag labels (max 5, ", shapes::RESOURCE_TAGS_MATCH, ")")),
            shapes::TagsMatch,
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

    let (tags, tag_match) = query.filter()?;

    let order = query.order.unwrap_or(SortOrder::Descending);
    let pagination = query.pagination.to_pagination(query.start, query.end);

    let keys = ResourceStream::get_resource_keys(
        pagination,
        order,
        &query.sorting,
        tags.as_deref(),
        tag_match,
    )
    .await?;

    if keys.is_empty() {
        return Ok(Json(vec![]));
    }

    let stream = ResourceStream::from_listed_resource_ids(
        query.viewer_id.as_deref(),
        &keys.resource_ids,
        V::REFERENCE_COUNTS,
    )
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
            ("tags" = Option<shapes::Tags>, Query, description = concatcp!("Comma-separated tag labels (max 5, ", shapes::RESOURCE_TAGS_MATCH, ")")),
            shapes::TagsMatch,
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

/// Supply exactly one list of 1–100 entries. URIs are normalized as `/v1/resource/by-uri` normalizes them.
///
/// Its schema is written by hand ([`PartialSchema`] below): a derived one would mark both
/// lists optional and nullable, which documents bodies the handler rejects.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceStreamByIdsRequest {
    #[serde(default, deserialize_with = "resource_id_list")]
    resource_ids: Option<Vec<ResourceId>>,
    #[serde(default, deserialize_with = "uri_list")]
    uris: Option<Vec<String>>,
    viewer_id: Option<PubkyId>,
}

/// The body is one of two shapes: `resource_ids` alone or `uris` alone, each a required,
/// non-null list of 1–100 entries, with an optional (or null) `viewer_id` and no other field.
impl PartialSchema for ResourceStreamByIdsRequest {
    fn schema() -> RefOr<Schema> {
        let shape = |list: &str, items: RefOr<Schema>, description: &str| {
            ObjectBuilder::new()
                .property(
                    list,
                    ArrayBuilder::new()
                        .items(items)
                        .min_items(Some(1))
                        .max_items(Some(100))
                        .description(Some(description)),
                )
                .required(list)
                .property(
                    "viewer_id",
                    OneOfBuilder::new()
                        .item(ObjectBuilder::new().schema_type(Type::Null))
                        .item(Ref::from_schema_name(PubkyId::name())),
                )
                .additional_properties(Some(AdditionalProperties::FreeForm(false)))
        };
        OneOfBuilder::new()
            .item(shape(
                "resource_ids",
                Ref::from_schema_name(ResourceId::name()).into(),
                "Resource ids, each 32-char lowercase hex",
            ))
            .item(shape(
                "uris",
                ObjectBuilder::new().schema_type(Type::String).into(),
                "Each URI must be at most 2048 bytes and have a valid scheme",
            ))
            .description(Some(
                "Supply exactly one list of 1–100 entries: `resource_ids` or `uris`. URIs are normalized as `/v1/resource/by-uri` normalizes them.",
            ))
            .into()
    }
}

impl ToSchema for ResourceStreamByIdsRequest {
    fn schemas(schemas: &mut Vec<(String, RefOr<Schema>)>) {
        schemas.push((ResourceId::name().into(), ResourceId::schema()));
        <ResourceId as ToSchema>::schemas(schemas);
        schemas.push((PubkyId::name().into(), PubkyId::schema()));
        <PubkyId as ToSchema>::schemas(schemas);
    }
}

/// A present `resource_ids`: 1–100 valid ids. Absent, the field is `None`; `null` is rejected.
fn resource_id_list<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<Vec<ResourceId>>, D::Error> {
    deserialize_json_array::<ResourceId, _, 1, 100>(deserializer).map(Some)
}

/// A present `uris`: 1–100 strings. Absent, the field is `None`; `null` is rejected.
fn uri_list<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<Vec<String>>, D::Error> {
    deserialize_json_array::<String, _, 1, 100>(deserializer).map(Some)
}

impl ResourceStreamByIdsRequest {
    fn into_ids(self) -> Result<(Vec<String>, Option<PubkyId>)> {
        let ids = match (self.resource_ids, self.uris) {
            (Some(ids), None) => ids.into_iter().map(|id| id.0).collect(),
            (None, Some(uris)) => uris
                .into_iter()
                .map(|uri| {
                    if uri.len() > MAX_URI_LENGTH {
                        return Err(Error::invalid_input(format!(
                            "URI exceeds {MAX_URI_LENGTH} bytes"
                        )));
                    }
                    let (normalized, _) = normalize_uri(&uri).map_err(Error::invalid_input)?;
                    Ok(resource_id(&normalized))
                })
                .collect::<Result<Vec<_>>>()?,
            (Some(_), Some(_)) => {
                return Err(Error::invalid_input(
                    "Supply resource_ids or uris, not both",
                ))
            }
            (None, None) => return Err(Error::invalid_input("Supply resource_ids or uris")),
        };
        Ok((ids, self.viewer_id))
    }
}

/// `POST /v1/stream/resources/by_ids`, a route only `/v1` serves (`V1::own_routes`): `/v0` is
/// frozen. It answers the `/v1` resource views, whose tags carry `tag_uri` and no
/// `relationship`.
#[utoipa::path(
    post,
    path = STREAM_RESOURCES_BY_IDS_ROUTE,
    tag = "Stream",
    description = "Load 1–100 resources by resource_ids or uris (exactly one list). URIs are normalized and hashed as /v1/resource/by-uri does. Results follow request order, including duplicates; unknown resources are skipped. Each view carries details.id. viewer_id fills each tag's tag_uri with the viewer's tag file, if any. No resource is fetched or created by this lookup.",
    request_body = ResourceStreamByIdsRequest,
    responses(
        (status = 200, description = "Resource views in request order; an empty array when nothing matches", body = Vec<v1::ResourceView>),
        (status = 400, description = "Invalid body, ID, URI, viewer, or list length"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_resources_by_ids_handler(
    RequestJson(request): RequestJson<ResourceStreamByIdsRequest>,
) -> Result<Json<Vec<v1::ResourceView>>> {
    let (ids, viewer_id) = request.into_ids()?;
    let views = ResourceStream::from_listed_resource_ids(
        viewer_id.as_deref(),
        &ids,
        <V1 as ApiVersion>::REFERENCE_COUNTS,
    )
    .await?
    .map(|stream| stream.0)
    .unwrap_or_default()
    .into_iter()
    .map(v1::ResourceView::from)
    .collect();
    Ok(Json(views))
}

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
    fn names_the_reason_a_body_is_rejected() {
        let ids: Vec<&str> = vec![ID; 101];
        for (body, reason) in [
            (json!({"resource_ids": ids}), "Maximum 100 items allowed"),
            (json!({"resource_ids": []}), "At least 1 item(s) required"),
            (
                json!({"resource_ids": [ID.to_uppercase()]}),
                "32-char lowercase hex",
            ),
            (
                json!({"resource_ids": [ID], "viewer_id": "invalid"}),
                "52 ASCII characters",
            ),
            (
                json!({"resource_ids": [ID], "uris": ["https://example.com"]}),
                "not both",
            ),
            (json!({}), "Supply resource_ids or uris"),
            (
                json!({"resource_ids": [ID], "unknown": true}),
                "unknown field",
            ),
        ] {
            let error = validate(body.clone()).expect_err("rejected").to_string();
            assert!(error.contains(reason), "{body}: {error}");
        }
    }

    /// A bad id's rejection echoes a bounded prefix, not the whole entry.
    #[test]
    fn a_long_bad_id_gets_a_short_rejection() {
        let error = validate(json!({"resource_ids": ["a".repeat(10_000)]}))
            .expect_err("rejected")
            .to_string();
        assert!(error.contains("32-char lowercase hex"), "{error}");
        assert!(error.len() < 300, "{} bytes: {error}", error.len());
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
