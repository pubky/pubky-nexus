use crate::routes::v0::endpoints::TAG_TRENDING_ROUTE;
use crate::{register_routes, Error, Result};
use axum::extract::Path;
use axum::{Json, Router};
use log::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = TAG_TRENDING_ROUTE,
    tag = "Trending (hot) tags",
    responses(
        (status = 200, description = "Hot Tags", body = Post),
        (status = 404, description = "Tags not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn trending_tags_handler() -> Result<Json<>> {
    info!( "GET {TAG_TRENDING_ROUTE}");
    Ok(Json(post))
}

pub fn routes() -> Router {
    register_routes!(Router::new(), super::endpoints::TAG_TRENDING_ROUTE => trending_tags_handler)
}

#[derive(OpenApi)]
#[openapi(paths(trending_tags_handler), components(schemas()))]
pub struct TrendingTagsApiDoc;
