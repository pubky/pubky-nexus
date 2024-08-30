use crate::models::tag::post::TagPost;
use crate::models::tag::traits::TagCollection;
use crate::models::tag::TagDetails;
use crate::routes::v0::endpoints::POST_TAGS_ROUTE;
use crate::routes::v0::TagsQuery;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use log::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = POST_TAGS_ROUTE,
    tag = "Post Tags",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("post_id" = String, Path, description = "Post ID"),
        ("limit_tags" = Option<usize>, Query, description = "Upper limit on the number of tags for the post"),
        ("limit_taggers" = Option<usize>, Query, description = "Upper limit on the number of taggers per tag")
    ),
    responses(
        (status = 200, description = "Post tags", body = TagPost),
        (status = 404, description = "Post not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn post_tags_handler(
    Path((user_id, post_id)): Path<(String, String)>,
    Query(query): Query<TagsQuery>,
) -> Result<Json<Vec<TagDetails>>> {
    info!(
        "GET {POST_TAGS_ROUTE} user_id:{}, post_id: {}, limit_tags:{:?}, limit_taggers:{:?}",
        user_id, post_id, query.limit_tags, query.limit_taggers
    );
    match TagPost::get_by_id(
        &user_id,
        Some(&post_id),
        query.limit_tags,
        query.limit_taggers,
    )
    .await
    {
        Ok(Some(tags)) => Ok(Json(tags)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(post_tags_handler), components(schemas(TagDetails)))]
pub struct PostTagsApiDoc;
