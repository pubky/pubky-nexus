use crate::models::post::PostCounts;
use crate::routes::v0::endpoints::SWAGGER_POST_COUNTS_ROUTE;
use crate::{Error, Result};
use axum::extract::Path;
use axum::Json;
use log::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = SWAGGER_POST_COUNTS_ROUTE,
    tag = "Post Counts",
    params(
        ("author_id" = String, Path, description = "Author Pubky ID"),
        ("post_id" = String, Path, description = "Post Crockford32 ID")
    ),
    responses(
        (status = 200, description = "Post Counts", body = PostCounts),
        (status = 404, description = "Post not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn post_counts_handler(
    Path((author_id, post_id)): Path<(String, String)>,
) -> Result<Json<PostCounts>> {
    info!(
        "GET {SWAGGER_POST_COUNTS_ROUTE} author_id:{}, post_id:{}",
        author_id, post_id
    );

    match PostCounts::get_by_id(&author_id, &post_id).await {
        Ok(Some(post)) => Ok(Json(post)),
        Ok(None) => Err(Error::PostNotFound { author_id, post_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(post_counts_handler), components(schemas(PostCounts)))]
pub struct PostCountsApiDoc;
