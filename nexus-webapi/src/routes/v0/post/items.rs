use crate::models::{BoundedLimit, PostId, PubkyId};
use crate::routes::v0::endpoints::POST_ITEMS_ROUTE;
use crate::routes::v0::post::view::PostPath;
use crate::routes::{Path, Query};
use crate::{Error, Result};
use axum::Json;
use nexus_common::models::post::PostKind;
use nexus_common::models::post::{
    collection_items_page, CollectionItem, CollectionItemsPage, PostDetails,
};
use serde::Deserialize;
use tracing::{debug, warn};
use utoipa::OpenApi;

#[derive(Deserialize, Debug, Default)]
pub struct CollectionItemsQuery {
    /// `next_cursor` of the previous page; the first page without it.
    pub cursor: Option<String>,
    pub limit: Option<BoundedLimit<20, 100>>,
}

#[utoipa::path(
    get,
    path = POST_ITEMS_ROUTE,
    description = "The items of a Collection post in curator order, posts and external resources alike, one bounded page at a time. Each item carries its address, note, and either `post_key` (a Pubky post) or `resource_id` (any other address, the id `/v0/resource/{resource_id}` takes). Hand `next_cursor` back as `cursor` until it is null; the cursor has no ceiling, so a Collection of any length reads to its end. Hydrate the page with `POST /v0/stream/posts/by_ids` and `POST /v0/stream/resources/by_ids`.",
    tag = "Post",
    params(
        ("author_id" = PubkyId, Path, description = "Curator Pubky ID"),
        ("post_id" = PostId, Path, description = "Collection post ID"),
        ("cursor" = Option<String>, Query, description = "`next_cursor` of the previous page"),
        ("limit" = Option<BoundedLimit<20, 100>>, Query, description = "Items per page (1–100, default 20)"),
    ),
    responses(
        (status = 200, description = "A page of items", body = CollectionItemsPage),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "Collection not found"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn collection_items_handler(
    Path(PostPath { author_id, post_id }): Path<PostPath>,
    Query(query): Query<CollectionItemsQuery>,
) -> Result<Json<CollectionItemsPage>> {
    debug!("GET {POST_ITEMS_ROUTE} author_id:{author_id}, post_id:{post_id}");
    let cursor = match query.cursor.as_deref() {
        None => 0,
        Some(cursor) => cursor
            .parse::<usize>()
            .map_err(|_| Error::invalid_input("`cursor` must be a `next_cursor` value"))?,
    };
    let limit = query.limit.map(|limit| limit.value()).unwrap_or(20);
    let Some(details) = PostDetails::get_by_id(&author_id, &post_id)
        .await?
        .filter(|post| post.kind == PostKind::Collection && !post.deleted)
    else {
        return Err(Error::post_not_found(author_id, post_id));
    };
    // A malformed envelope lists no items, as `source=collection` treats it.
    let page = collection_items_page(&details.content, cursor, limit).unwrap_or_else(|e| {
        warn!("Collection {author_id}:{post_id} envelope malformed: {e}");
        CollectionItemsPage::default()
    });
    Ok(Json(page))
}

#[derive(OpenApi)]
#[openapi(
    paths(collection_items_handler),
    components(schemas(CollectionItemsPage, CollectionItem))
)]
pub struct CollectionItemsApiDoc;
