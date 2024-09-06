use crate::models::post::{PostRelationships, PostView};
use crate::models::tag::post::TagPost;
use crate::models::tag::TagDetails;
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
    max_tags: Option<usize>,
    max_taggers: Option<usize>,
}

#[utoipa::path(
    get,
    path = POST_ROUTE,
    tag = "Post",
    params(
        ("author_id" = String, Path, description = "Author Pubky ID"),
        ("post_id" = String, Path, description = "Post Crockford32 ID"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("max_tags" = Option<usize>, Query, description = "Upper limit on the number of tags for the post"),
        ("max_taggers" = Option<usize>, Query, description = "Upper limit on the number of taggers per tag")
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
        "GET {POST_ROUTE} author_id:{}, post_id:{}, viewer_id:{}, max_tags:{:?}, max_taggers:{:?}",
        author_id,
        post_id,
        query.viewer_id.clone().unwrap_or_default(),
        query.max_tags,
        query.max_taggers
    );

    match PostView::get_by_id(
        &author_id,
        &post_id,
        query.viewer_id.as_deref(),
        query.max_tags,
        query.max_taggers,
    )
    .await
    {
        Ok(Some(post)) => Ok(Json(post)),
        Ok(None) => Err(Error::PostNotFound { author_id, post_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(post_view_handler),
    components(schemas(PostView, PostRelationships, TagPost, TagDetails))
)]
pub struct PostViewApiDoc;
