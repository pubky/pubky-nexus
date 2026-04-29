use crate::models::{PostId, PubkyId};
use crate::routes::v0::endpoints::POST_BOOKMARK_ROUTE;
use crate::routes::v0::post::view::PostPath;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use nexus_common::models::post::Bookmark;
use serde::Deserialize;
use tracing::debug;
use utoipa::OpenApi;

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
        (status = 404, description = "Post not found"),
        (status = 500, description = "Internal server error")
    )
)]
//todo: test it manually - no integration tests
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
        None => Err(Error::PostNotFound { author_id, post_id }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(post_bookmark_handler),
    components(schemas(Bookmark, PubkyId, PostId))
)]
pub struct BookmarkApiDoc;
