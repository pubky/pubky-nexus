use crate::routes::v0::endpoints::STREAM_TAGS_HOT_ROUTE;
use crate::{Error, Result};
use axum::extract::Query;
use axum::Json;
use log::info;
use serde::Deserialize;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = STREAM_TAGS_HOT_ROUTE,
    tag = "Posts hot tags",
    responses(
        (status = 200, description = "Post hot tags stream", body = PostStream),
        (status = 404, description = "Hot tags not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_hot_tags_handler() {
    info!("GET {STREAM_TAGS_HOT_ROUTE}");

}

#[derive(OpenApi)]
#[openapi(
    paths(
        stream_hot_tags_handler
    ),
    components(schemas())
)]
pub struct StreamTagsApiDocs;
