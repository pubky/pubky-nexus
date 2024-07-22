use crate::models::post::Post;
use crate::routes::v0::endpoints::POST_ROUTE;
use crate::{register_routes, Error, Result};
use axum::extract::Path;
use axum::{Json, Router};
use log::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = POST_ROUTE,
    tag = "Post",
    params(
        ("author_id" = String, Path, description = "Author Pubky ID"),
        ("post_id" = String, Path, description = "Post Crockford32 ID")
    ),
    responses(
        (status = 200, description = "Post", body = Post),
        (status = 404, description = "Post not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn post_handler(
    Path((author_id, post_id)): Path<(String, String)>,
) -> Result<Json<Post>> {
    info!(
        "GET {POST_ROUTE} author_id:{}, post_id:{}",
        author_id, post_id
    );

    match Post::get_by_id(&author_id, &post_id).await {
        Ok(Some(post)) => Ok(Json(post)),
        Ok(None) => Err(Error::PostNotFound { author_id, post_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

pub fn routes() -> Router {
    register_routes!(Router::new(), super::endpoints::POST_ROUTE => post_handler)
}

#[derive(OpenApi)]
#[openapi(paths(post_handler), components(schemas(Post)))]
pub struct PostApiDoc;
