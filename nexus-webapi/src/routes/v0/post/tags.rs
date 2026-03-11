use crate::routes::v0::endpoints::{POST_TAGGERS_ROUTE, POST_TAGS_ROUTE};
use crate::routes::v0::user::tags::TaggersQuery;
use crate::routes::v0::{TaggersInfoResponse, TagsQuery};
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use nexus_common::models::tag::post::TagPost;
use nexus_common::models::tag::traits::{TagCollection, TaggersCollection};
use nexus_common::models::tag::TagDetails;
use tracing::debug;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = POST_TAGS_ROUTE,
    description = "Post tags",
    tag = "Post",
    params(
        ("author_id" = String, Path, description = "Author Pubky ID"),
        ("post_id" = String, Path, description = "Post ID"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("skip_tags" = Option<usize>, Query, description = "Skip N tags. Defaults to `0`"),
        ("limit_tags" = Option<usize>, Query, description = "Upper limit on the number of tags for the posts. Defaults to `5`"),
        ("limit_taggers" = Option<usize>, Query, description = "Upper limit on the number of taggers per tag. Defaults to `5`"),
    ),
    responses(
        (status = 404, description = "Post not found"),
        (status = 200, description = "Post tags", body = Vec<TagDetails>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn post_tags_handler(
    Path((author_id, post_id)): Path<(String, String)>,
    Query(query): Query<TagsQuery>,
) -> Result<Json<Vec<TagDetails>>> {
    debug!(
        "GET {POST_TAGS_ROUTE} author_id:{}, post_id: {}, skip_tags:{:?}, limit_tags:{:?}, limit_taggers:{:?}",
        author_id, post_id, query.limit_tags, query.skip_tags, query.limit_taggers
    );
    match TagPost::get_by_id(
        &author_id,
        Some(&post_id),
        query.skip_tags,
        query.limit_tags,
        query.limit_taggers,
        query.viewer_id.as_deref(),
        None, // Avoid by default WoT tags in a Post
    )
    .await?
    {
        Some(tags) => Ok(Json(tags)),
        None => Err(Error::PostNotFound { author_id, post_id }),
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
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("skip" = Option<usize>, Query, description = "Number of taggers to skip for pagination. Defaults to `0`"),
        ("limit" = Option<usize>, Query, description = "Number of taggers to return for pagination. Defaults to `40`")
    ),
    responses(
        (status = 200, description = "Post tags", body = TaggersInfoResponse),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn post_taggers_handler(
    Path((author_id, post_id, label)): Path<(String, String, String)>,
    Query(taggers_query): Query<TaggersQuery>,
) -> Result<Json<TaggersInfoResponse>> {
    debug!(
        "GET {POST_TAGGERS_ROUTE} author_id:{}, post_id: {}, label: {}, viewer_id:{:?}, skip:{:?}, limit:{:?}",
        author_id, post_id, label, taggers_query.tags_query.viewer_id, taggers_query.pagination.skip, taggers_query.pagination.limit
    );
    let taggers = TagPost::get_tagger_by_id(
        &author_id,
        Some(&post_id),
        &label,
        taggers_query.pagination,
        taggers_query.tags_query.viewer_id.as_deref(),
        None,
    )
    .await?;
    Ok(Json(TaggersInfoResponse::from(taggers)))
}

#[derive(OpenApi)]
#[openapi(
    paths(post_tags_handler, post_taggers_handler),
    components(schemas(TagDetails, TaggersInfoResponse))
)]
pub struct PostTagsApiDoc;
