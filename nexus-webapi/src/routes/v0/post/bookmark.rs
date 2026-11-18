//! The viewer's bookmark of a post. Served and documented by `/v0` alone (`V0::own_routes`,
//! `V0Doc`): `social/v1` bookmarks live under `/priv/`, which Nexus never sees, so `/v1` has no
//! bookmark route. The graph edges, the Redis keys and this route stay until R1.

use crate::models::{PostId, PubkyId};
use crate::routes::v0::endpoints::POST_BOOKMARK_ROUTE;
use crate::routes::v0::post::view::PostPath;
use crate::routes::Path;
use crate::routes::Query;
use crate::{Error, Result};
use axum::Json;
use nexus_common::models::post::Bookmark;
use serde::Deserialize;
use tracing::debug;

#[derive(Deserialize)]
pub struct PostQuery {
    viewer_id: Option<PubkyId>,
}

#[utoipa::path(
    get,
    path = POST_BOOKMARK_ROUTE,
    description = "Post bookmark",
    tag = "Post",
    params(
        ("author_id" = PubkyId, Path, description = "Author Pubky ID"),
        ("post_id" = PostId, Path, description = "Post Crockford32 ID"),
        ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID")
    ),
    responses(
        (status = 200, description = "Post Bookmark", body = Bookmark),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "Post not found"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn post_bookmark_handler(
    Path(PostPath { author_id, post_id }): Path<PostPath>,
    Query(query): Query<PostQuery>,
) -> Result<Json<Bookmark>> {
    debug!(
        "GET {POST_BOOKMARK_ROUTE} author_id:{}, post_id:{}, viewer_id:{:?}",
        author_id, post_id, query.viewer_id,
    );

    match Bookmark::get_by_id(&author_id, &post_id, query.viewer_id.as_deref()).await? {
        Some(post) => Ok(Json(post)),
        None => Err(Error::post_not_found(author_id, post_id)),
    }
}
