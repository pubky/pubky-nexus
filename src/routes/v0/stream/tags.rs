use crate::models::tag::stream::StreamTags;
use crate::models::user::UserStreamType;
use crate::routes::v0::endpoints::{STREAM_TAGS_GLOBAL_ROUTE, STREAM_REACHED_TAGS_ROUTE};
use crate::{Error, Result};
use axum::extract::Path;
use axum::Json;
use log::{ info, error };
use serde::Deserialize;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = STREAM_TAGS_GLOBAL_ROUTE,
    tag = "Posts hot tags",
    responses(
        (status = 200, description = "Retrieve hot tags stream", body = Vec<StreamTags>),
        (status = 404, description = "Hot tags not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_hot_tags_handler() -> Result<Json<Vec<StreamTags>>> {
    info!("GET {STREAM_TAGS_GLOBAL_ROUTE}");
    
    match StreamTags::get_global_tags_stream().await {
        Ok(Some(hot_tags)) => Ok(Json(hot_tags)),
        Ok(None) => Err(Error::TagsNotFound { reach: String::from("GLOBAL") }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(Deserialize)]
pub struct StreamTagsReachInput {
    user_id: String,
    reach: Option<UserStreamType>,
}

#[utoipa::path(
    get,
    path = STREAM_REACHED_TAGS_ROUTE,
    tag = "Posts reached graph tags",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("reach" = UserStreamType, Path, description = "Reach type: Follower | Following | Friends")
    ),
    responses(
        (status = 200, description = "Retrieve tags from the reached graph", body = StreamTags),
        (status = 404, description = "Hot tags not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_reached_tags_handler(
    Path(path): Path<StreamTagsReachInput>,
) -> Result<Json<Vec<StreamTags>>> {
    info!("GET {STREAM_REACHED_TAGS_ROUTE}");

    let reach = path.reach.unwrap_or(UserStreamType::Following);
    let user_id = path.user_id;
    
    match StreamTags::get_stream_tags_from_reached(user_id, reach).await {
        Ok(Some(hot_tags)) => Ok(Json(hot_tags)),
        Ok(None) => Err(Error::TagsNotFound { reach: String::from("REACH") }),
        Err(source) => {
            error!("Internal Server ERROR: {:?}", source);
            Err(Error::InternalServerError { source })
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        stream_hot_tags_handler,
        stream_reached_tags_handler
    ),
    components(schemas(StreamTags))
)]
pub struct StreamTagsApiDocs;
