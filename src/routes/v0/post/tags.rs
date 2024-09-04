use crate::models::tag::post::TagPost;
use crate::models::tag::traits::taggers::Taggers;
use crate::models::tag::traits::{TagCollection, TaggersCollection};
use crate::models::tag::TagDetails;
use crate::routes::v0::endpoints::{POST_TAGGERS_ROUTE, POST_TAGS_ROUTE};
use crate::routes::v0::queries::PaginationQuery;
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
        ("skip" = Option<usize>, Query, description = "Upper limit on the number of tags for the post"),
        ("limit" = Option<usize>, Query, description = "Upper limit on the number of taggers per tag")
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
    match TagPost::try_from_multiple_index(
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

#[utoipa::path(
    get,
    path = POST_TAGGERS_ROUTE,
    tag = "Post specific label Taggers",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("label" = String, Path, description = "Tag name"),
        ("post_id" = String, Path, description = "Post ID"),
        ("skip" = Option<usize>, Query, description = "Number of taggers to skip for pagination"),
        ("limit" = Option<usize>, Query, description = "Number of taggers to return for pagination")
    ),
    responses(
        (status = 200, description = "Post tags", body = TagPost),
        (status = 404, description = "Post not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn post_taggers_handler(
    Path((user_id, post_id, label)): Path<(String, String, String)>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Taggers>> {
    info!(
        "GET {POST_TAGGERS_ROUTE} user_id:{}, post_id: {}, label: {}, skip:{:?}, limit:{:?}",
        user_id, post_id, label, query.skip, query.limit
    );
    match TagPost::try_from_index(&user_id, Some(&post_id), &label, query.skip, query.limit).await {
        Ok(Some(tags)) => Ok(Json(tags)),
        Ok(None) => Err(Error::UserNotFound { user_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(post_tags_handler, post_taggers_handler),
    components(schemas(TagDetails))
)]
pub struct PostTagsApiDoc;
