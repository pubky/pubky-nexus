use crate::models::tag::stream::StreamTags;
use crate::routes::v0::endpoints::STREAM_TAGS_GLOBAL_ROUTE;
use crate::{Error, Result};
use axum::Json;
use log::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = STREAM_TAGS_GLOBAL_ROUTE,
    tag = "Posts hot tags",
    responses(
        (status = 200, description = "Retrieve hot tags stream", body = StreamTags),
        (status = 404, description = "Hot tags not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_hot_tags_handler() -> Result<Json<Vec<StreamTags>>>{
    info!("GET {STREAM_TAGS_GLOBAL_ROUTE}");
    
    match StreamTags::get_global_tags_stream().await {
        Ok(Some(hot_tags)) => Ok(Json(hot_tags)),
        Ok(None) => Err(Error::TagsNotFound { reach: String::from("GLOBAL") }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        stream_hot_tags_handler
    ),
    components(schemas(StreamTags))
)]
pub struct StreamTagsApiDocs;
