use crate::routes::v0::endpoints::POST_ROUTE;
use crate::routes::v0::TagsQuery;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use nexus_common::models::post::{PostRelationships, PostView};
use nexus_common::models::tag::post::TagPost;
use nexus_common::models::tag::TagDetails;
use tracing::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = POST_ROUTE,
    description = "Post view",
    tag = "Post",
    params(
        ("author_id" = String, Path, description = "Author Pubky ID"),
        ("post_id" = String, Path, description = "Post Crockford32 ID"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("limit_tags" = Option<usize>, Query, description = "Upper limit on the number of tags for the post"),
        ("limit_taggers" = Option<usize>, Query, description = "Upper limit on the number of taggers per tag")
    ),
    responses(
        (status = 200, description = "Post", body = PostView),
        (status = 404, description = "Post not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn post_view_handler(
    Path((author_id, post_id)): Path<(String, String)>,
    Query(query): Query<TagsQuery>,
) -> Result<Json<PostView>> {
    info!(
        "GET {POST_ROUTE} author_id:{}, post_id:{}, viewer_id:{}, limit_tags:{:?}, limit_taggers:{:?}",
        author_id,
        post_id,
        query.viewer_id.clone().unwrap_or_default(),
        query.limit_tags,
        query.limit_taggers
    );
    // Avoid by default WoT tags in a Post. We could add as `depth` argument for that specific use case
    match PostView::get_by_id(
        &author_id,
        &post_id,
        query.viewer_id.as_deref(),
        query.limit_tags,
        query.limit_taggers,
    )
    .await
    {
        Ok(Some(post)) => Ok(Json(post)),
        Ok(None) => Err(Error::PostNotFound { author_id, post_id }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(post_view_handler),
    components(schemas(PostView, PostRelationships, TagPost, TagDetails))
)]
pub struct PostViewApiDoc;
