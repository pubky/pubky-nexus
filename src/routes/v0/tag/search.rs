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
    tag = "Search tags by name",
    params(
        ("tag_name" = String, Path, description = "Tag name")
    ),
    responses(
        (status = 200, description = "Find tags by name", body = Post),
        (status = 404, description = "Tags not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn search_tags_handler(
    Path(tag_name): Path<String>
) -> Result<Json<>> {
    info!(
        "GET {POST_ROUTE} tag_name:{}",
        tag_name
    );
}

pub fn routes() -> Router {
    register_routes!(Router::new(), super::endpoints::TAG_SEARCH_ROUTE => search_tags_handler)
}

#[derive(OpenApi)]
#[openapi(paths(search_tags_handler), components(schemas(Post)))]
pub struct SearchTagApiDoc;
