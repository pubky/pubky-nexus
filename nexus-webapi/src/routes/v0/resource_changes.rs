use super::endpoints::RESOURCE_CHANGES_ROUTE;
use crate::models::BoundedLimit;
use crate::routes::{AppState, Query};
use crate::Result;
use axum::http::{HeaderMap, HeaderValue};
use axum::{routing::get, Json, Router};
use nexus_common::models::event::resource_changes::{ResourceChange, ResourceChangesPage};
use serde::Deserialize;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct ResourceChangesQuery {
    cursor: Option<u64>,
    limit: Option<BoundedLimit<500, 1000>>,
}

#[utoipa::path(
    get,
    path = RESOURCE_CHANGES_ROUTE,
    tag = "Resource",
    description = "Reference feed for accepted non-Post, non-User targets. Currently emits only tag put/del; reply, embed and collection are reserved. Read-only: never fetches resources or seeds the feed. Delivery contract: lines are delivered at least once, in append order. A consumer must apply put as an upsert keyed by (author, object_path) and del as a delete by the same key. Duplicates are expected after retries and must be harmless. On an epoch change or a requested cursor greater than x-events-length, restart from cursor 0 and rebuild local reference state. Persist cursor and epoch together. Do not compare next_cursor to a length from a different response. x-events-epoch is absent until the first append or seed; retain the previous epoch when absent. This feed has its own epoch and uses the header names from PR #1125 without depending on that PR.",
    params(
        ("cursor" = Option<u64>, Query, description = "Zero-based list offset; default 0"),
        ("limit" = Option<BoundedLimit<500, 1000>>, Query, description = "Page size, 1–1000; default 500")
    ),
    responses(
        (status = 200, description = "Resource changes and next cursor", body = ResourceChangesPage,
            headers(
                ("x-events-epoch" = String, description = "Opaque feed generation; absent before initialization. Changes on reseed or a new feed."),
                ("x-events-length" = u64, description = "List length from the same atomic snapshot as the page")
            )),
        (status = 400, description = "Invalid cursor or limit"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn resource_changes_handler(
    Query(query): Query<ResourceChangesQuery>,
) -> Result<(HeaderMap, Json<ResourceChangesPage>)> {
    let (page, epoch, length) = ResourceChange::get_from_index(
        query.cursor.unwrap_or(0),
        query.limit.map_or(500, |limit| limit.value()),
    )
    .await?;
    let mut headers = HeaderMap::new();
    headers.insert("x-events-length", HeaderValue::from(length));
    if let Some(epoch) = epoch {
        headers.insert("x-events-epoch", HeaderValue::from_str(&epoch)?);
    }
    Ok((headers, Json(page)))
}

pub fn routes() -> Router<AppState> {
    Router::new().route(RESOURCE_CHANGES_ROUTE, get(resource_changes_handler))
}

#[derive(OpenApi)]
#[openapi(paths(resource_changes_handler))]
pub struct ResourceChangesApiDoc;
