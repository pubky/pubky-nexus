use crate::routes::v0::endpoints::POST_BOOKMARK_ROUTE;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use nexus_common::models::post::Bookmark;
use serde::Deserialize;
use tracing::info;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct PostQuery {
    viewer_id: Option<String>,
}

#[utoipa::path(
    get,
    path = POST_BOOKMARK_ROUTE,
    description = "Post bookmark",
    tag = "Post",
    params(
        ("author_id" = String, Path, description = "Author Pubky ID"),
        ("post_id" = String, Path, description = "Post Crockford32 ID"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID")
    ),
    responses(
        (status = 200, description = "Post Bookmark", body = Bookmark),
        (status = 404, description = "Post not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn post_bookmark_handler(
    Path((author_id, post_id)): Path<(String, String)>,
    Query(query): Query<PostQuery>,
) -> Result<Json<Bookmark>> {
    info!(
        "GET {POST_BOOKMARK_ROUTE} author_id:{}, post_id:{}, viewer_id:{}",
        author_id,
        post_id,
        query.viewer_id.clone().unwrap_or_default()
    );

    match Bookmark::get_by_id(&author_id, &post_id, query.viewer_id.as_deref()).await {
        Ok(Some(post)) => Ok(Json(post)),
        Ok(None) => Err(Error::PostNotFound { author_id, post_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(post_bookmark_handler), components(schemas(Bookmark)))]
pub struct BookmarkApiDoc;
