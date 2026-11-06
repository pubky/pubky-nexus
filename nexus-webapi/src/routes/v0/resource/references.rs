use crate::models::{
    BoundedLimit, BoundedPagination, BoundedSkip, PostViewDetailed, PubkyId, ResourceId,
};
use crate::routes::v0::endpoints::{RESOURCE_EMBEDS_ROUTE, RESOURCE_REPLIES_ROUTE};
use crate::routes::v0::post::view::PostViewQuery;
use crate::routes::{Path, Query};
use crate::{Error, Result};
use axum::Json;
use nexus_common::models::event::resource_changes::ResourceChangeKind;
use nexus_common::models::resource::references;
use serde::Deserialize;

#[derive(Deserialize, Default)]
pub struct ReferencesQuery {
    #[serde(flatten)]
    pub pagination: BoundedPagination<10_000, 20, 100>,
    #[serde(flatten)]
    pub view: PostViewQuery,
}

async fn read(
    id: ResourceId,
    query: ReferencesQuery,
    kind: ResourceChangeKind,
) -> Result<Json<Vec<PostViewDetailed>>> {
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
    let mut posts = Vec::with_capacity(keys.len());
    for (author, id) in keys {
        if let Some(post) = PostViewDetailed::get_by_id(
            &author,
            &id,
            query.view.viewer_id.as_deref(),
            query.view.limit_tags.as_ref().map(|n| n.value()),
            query.view.limit_taggers.as_ref().map(|n| n.value()),
            query.view.include_attachment_metadata,
        )
        .await?
        {
            posts.push(post);
        }
    }
    Ok(Json(posts))
}

#[utoipa::path(get, path = RESOURCE_REPLIES_ROUTE, tag = "Resource",
    description = "Live posts replying to a Resource, newest first with stable author/id ties. An existing Resource without replies returns an empty array.",
    params(("resource_id" = ResourceId, Path), ("skip" = Option<BoundedSkip<10_000>>, Query),
        ("limit" = Option<BoundedLimit<20, 100>>, Query), ("viewer_id" = Option<PubkyId>, Query),
        ("limit_tags" = Option<BoundedLimit<5, 100>>, Query), ("limit_taggers" = Option<BoundedLimit<5, 100>>, Query),
        ("include_attachment_metadata" = Option<bool>, Query)),
    responses((status=200, body=Vec<PostViewDetailed>, description="Replies"), (status=400, description="Invalid parameters"),
        (status=404, description="Resource not found"), (status=500, description="Internal server error")))]
pub async fn replies(
    Path(id): Path<ResourceId>,
    Query(query): Query<ReferencesQuery>,
) -> Result<Json<Vec<PostViewDetailed>>> {
    read(id, query, ResourceChangeKind::Reply).await
}

#[utoipa::path(get, path = RESOURCE_EMBEDS_ROUTE, tag = "Resource",
    description = "Live posts embedding a Resource, newest first with stable author/id ties. An existing Resource without embeds returns an empty array.",
    params(("resource_id" = ResourceId, Path), ("skip" = Option<BoundedSkip<10_000>>, Query),
        ("limit" = Option<BoundedLimit<20, 100>>, Query), ("viewer_id" = Option<PubkyId>, Query),
        ("limit_tags" = Option<BoundedLimit<5, 100>>, Query), ("limit_taggers" = Option<BoundedLimit<5, 100>>, Query),
        ("include_attachment_metadata" = Option<bool>, Query)),
    responses((status=200, body=Vec<PostViewDetailed>, description="Embeds"), (status=400, description="Invalid parameters"),
        (status=404, description="Resource not found"), (status=500, description="Internal server error")))]
pub async fn embeds(
    Path(id): Path<ResourceId>,
    Query(query): Query<ReferencesQuery>,
) -> Result<Json<Vec<PostViewDetailed>>> {
    read(id, query, ResourceChangeKind::Embed).await
}
