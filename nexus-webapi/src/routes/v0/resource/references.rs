//! `GET /v1/resource/{resource_id}/replies` and `GET /v1/resource/{resource_id}/embeds`: the
//! live posts that reply to or embed an external resource. `/v0` is frozen, so both are
//! mounted and documented with `/v1` alone (`V1::own_routes`, the v1 document), and answer
//! the `/v1` post views: `social/v1` kind names, no `bookmark`, no `attachments_metadata` and
//! tags without `relationship`.

use crate::models::{
    v1, BoundedLimit, BoundedPagination, BoundedSkip, PostViewDetailed, PubkyId, ResourceId,
};
use crate::routes::v0::endpoints::{RESOURCE_EMBEDS_ROUTE, RESOURCE_REPLIES_ROUTE};
use crate::routes::{Path, Query};
use crate::{Error, Result};
use axum::Json;
use futures_util::stream::{self, StreamExt, TryStreamExt};
use nexus_common::models::event::resource_changes::ResourceChangeKind;
use nexus_common::models::resource::references;
use serde::Deserialize;

/// The page and the post view options. No `include_attachment_metadata`: `/v1` has no file
/// details, and reads that name as any unknown parameter.
#[derive(Deserialize, Default)]
pub struct ReferencesQuery {
    #[serde(flatten)]
    pub pagination: BoundedPagination<10_000, 20, 100>,
    pub viewer_id: Option<PubkyId>,
    pub limit_tags: Option<BoundedLimit<5, 100>>,
    pub limit_taggers: Option<BoundedLimit<5, 100>>,
}

async fn read(
    id: ResourceId,
    query: ReferencesQuery,
    kind: ResourceChangeKind,
) -> Result<Json<Vec<v1::PostView>>> {
    if !references::exists(&id).await? {
        return Err(Error::resource_not_found(id.to_string()));
    }
    let keys = references::posts(
        &id,
        kind,
        query.pagination.skip_value(),
        query.pagination.limit_value(),
    )
    .await?;
    let posts = post_views(
        keys,
        query.viewer_id.as_deref(),
        query.limit_tags.as_ref().map(|n| n.value()),
        query.limit_taggers.as_ref().map(|n| n.value()),
    )
    .await?;
    Ok(Json(posts))
}

/// How many post views of a page load at once.
const PAGE_CONCURRENCY: usize = 8;

/// The `/v1` views of the posts `keys` names, `(author, post id)`, in the order of `keys`. A
/// page loads [`PAGE_CONCURRENCY`] views at a time, bounded to protect the pool as the post
/// streams are; a post with no view is left out.
pub async fn post_views(
    keys: Vec<(String, String)>,
    viewer_id: Option<&str>,
    limit_tags: Option<usize>,
    limit_taggers: Option<usize>,
) -> Result<Vec<v1::PostView>> {
    let views: Vec<Option<PostViewDetailed>> = stream::iter(keys)
        .map(|(author, id)| async move {
            PostViewDetailed::get_by_id(&author, &id, viewer_id, limit_tags, limit_taggers, false)
                .await
        })
        .buffered(PAGE_CONCURRENCY)
        .try_collect()
        .await?;
    Ok(views
        .into_iter()
        .flatten()
        .map(v1::PostView::from)
        .collect())
}

#[utoipa::path(get, path = RESOURCE_REPLIES_ROUTE, tag = "Resource",
    description = "Live posts replying to a Resource, newest first with stable author/id ties. An existing Resource without replies returns an empty array.",
    params(("resource_id" = ResourceId, Path), ("skip" = Option<BoundedSkip<10_000>>, Query),
        ("limit" = Option<BoundedLimit<20, 100>>, Query), ("viewer_id" = Option<PubkyId>, Query),
        ("limit_tags" = Option<BoundedLimit<5, 100>>, Query), ("limit_taggers" = Option<BoundedLimit<5, 100>>, Query)),
    responses((status=200, body=Vec<v1::PostView>, description="Replies"), (status=400, description="Invalid parameters"),
        (status=404, description="Resource not found"),
        (status=429, description="Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status=500, description="Internal server error")))]
pub async fn replies(
    Path(id): Path<ResourceId>,
    Query(query): Query<ReferencesQuery>,
) -> Result<Json<Vec<v1::PostView>>> {
    read(id, query, ResourceChangeKind::Reply).await
}

#[utoipa::path(get, path = RESOURCE_EMBEDS_ROUTE, tag = "Resource",
    description = "Live posts embedding a Resource, newest first with stable author/id ties. An existing Resource without embeds returns an empty array.",
    params(("resource_id" = ResourceId, Path), ("skip" = Option<BoundedSkip<10_000>>, Query),
        ("limit" = Option<BoundedLimit<20, 100>>, Query), ("viewer_id" = Option<PubkyId>, Query),
        ("limit_tags" = Option<BoundedLimit<5, 100>>, Query), ("limit_taggers" = Option<BoundedLimit<5, 100>>, Query)),
    responses((status=200, body=Vec<v1::PostView>, description="Embeds"), (status=400, description="Invalid parameters"),
        (status=404, description="Resource not found"),
        (status=429, description="Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status=500, description="Internal server error")))]
pub async fn embeds(
    Path(id): Path<ResourceId>,
    Query(query): Query<ReferencesQuery>,
) -> Result<Json<Vec<v1::PostView>>> {
    read(id, query, ResourceChangeKind::Embed).await
}
