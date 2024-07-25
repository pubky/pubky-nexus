use crate::models::post::{PostRelationships, PostView};
use crate::routes::v0::endpoints::POST_ROUTE;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use log::info;
use serde::Deserialize;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct PostQuery {
    viewer_id: Option<String>,
}

#[utoipa::path(
    get,
    path = POST_ROUTE,
    tag = "Post",
    params(
        ("author_id" = String, Path, description = "Author Pubky ID"),
        ("post_id" = String, Path, description = "Post Crockford32 ID"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID")
    ),
    responses(
        (status = 200, description = "Post", body = PostView),
        (status = 404, description = "Post not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn post_view_handler(
    Path((author_id, post_id)): Path<(String, String)>,
    Query(query): Query<PostQuery>,
) -> Result<Json<PostView>> {
    info!(
        "GET {POST_ROUTE} author_id:{}, post_id:{}, viewer_id:{}",
        author_id,
        post_id,
        query.viewer_id.clone().unwrap_or_default()
    );

    match PostView::get_by_id(&author_id, &post_id, query.viewer_id.as_deref()).await {
        Ok(Some(post)) => Ok(Json(post)),
        Ok(None) => Err(Error::PostNotFound { author_id, post_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(post_view_handler),
    components(schemas(PostView, PostRelationships))
)]
pub struct PostViewApiDoc;
