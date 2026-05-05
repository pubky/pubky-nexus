use crate::models::{PostId, PubkyId};
use crate::routes::v0::endpoints::POST_COUNTS_ROUTE;
use crate::routes::v0::post::view::PostPath;
use crate::routes::Path;
use crate::{Error, Result};
use axum::Json;
use nexus_common::models::post::PostCounts;
use tracing::debug;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = POST_COUNTS_ROUTE,
    description = "Post counts",
    tag = "Post",
    params(
        ("author_id" = PubkyId, Path, description = "Author Pubky ID"),
        ("post_id" = PostId, Path, description = "Post Crockford32 ID")
    ),
    responses(
        (status = 200, description = "Post Counts", body = PostCounts),
        (status = 404, description = "Post not found"),
        (status = 500, description = "Internal server error")
    )
)]
//todo: test it manually - no integration tests
pub async fn post_counts_handler(
    Path(PostPath { author_id, post_id }): Path<PostPath>,
) -> Result<Json<PostCounts>> {
    debug!(
        "GET {POST_COUNTS_ROUTE} author_id:{}, post_id:{}",
        author_id, post_id
    );

    match PostCounts::get_by_id(&author_id, &post_id).await? {
        Some(post) => Ok(Json(post)),
        None => Err(Error::PostNotFound { author_id, post_id }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(post_counts_handler),
    components(schemas(PostCounts, PubkyId, PostId))
)]
pub struct PostCountsApiDoc;
