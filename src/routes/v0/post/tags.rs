use crate::models::tag::post::TagPost;
use crate::models::tag::traits::taggers::Taggers;
use crate::models::tag::traits::{TagCollection, TaggersCollection};
use crate::models::tag::TagDetails;
use crate::routes::v0::endpoints::{POST_TAGGERS_ROUTE, POST_TAGS_ROUTE};
use crate::routes::v0::TagsQuery;
use crate::types::Pagination;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use log::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = POST_TAGS_ROUTE,
    description = "Post tags",
    tag = "Post",
    params(
        ("author_id" = String, Path, description = "Author Pubky ID"),
        ("post_id" = String, Path, description = "Post ID"),
        ("limit_tags" = Option<usize>, Query, description = "Upper limit on the number of tags for the posts"),
        ("limit_taggers" = Option<usize>, Query, description = "Upper limit on the number of taggers per tag"),
    ),
    responses(
        (status = 200, description = "Post tags", body = TagPost),
        (status = 404, description = "Post not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn post_tags_handler(
    Path((author_id, post_id)): Path<(String, String)>,
    Query(query): Query<TagsQuery>,
) -> Result<Json<Vec<TagDetails>>> {
    info!(
        "GET {POST_TAGS_ROUTE} author_id:{}, post_id: {}, limit_tags:{:?}, limit_taggers:{:?}",
        author_id, post_id, query.limit_tags, query.limit_taggers
    );
    match TagPost::get_by_id(
        &author_id,
        Some(&post_id),
        query.limit_tags,
        query.limit_taggers,
        None,
        None, // Avoid by default WoT tags in a Post
    )
    .await
    {
        Ok(Some(tags)) => Ok(Json(tags)),
        Ok(None) => Err(Error::PostNotFound { author_id, post_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[utoipa::path(
    get,
    path = POST_TAGGERS_ROUTE,
    description = "Post specific label Taggers",
    tag = "Post",
    params(
        ("author_id" = String, Path, description = "Author Pubky ID"),
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
    Path((author_id, post_id, label)): Path<(String, String, String)>,
    Query(pagination): Query<Pagination>,
) -> Result<Json<Taggers>> {
    info!(
        "GET {POST_TAGGERS_ROUTE} author_id:{}, post_id: {}, label: {}, skip:{:?}, limit:{:?}",
        author_id, post_id, label, pagination.skip, pagination.limit
    );
    match TagPost::get_tagger_by_id(&author_id, Some(&post_id), &label, pagination, None, None).await
    {
        Ok(Some(tags)) => Ok(Json(tags)),
        Ok(None) => Err(Error::PostNotFound { author_id, post_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(post_tags_handler, post_taggers_handler),
    components(schemas(TagDetails))
)]
pub struct PostTagsApiDoc;
