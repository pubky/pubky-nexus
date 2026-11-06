use crate::models::{BoundedLimit, PostId, PostViewDetailed, PubkyId};
use crate::routes::v0::endpoints::POST_ROUTE;
use crate::routes::v0::version::{versioned_handlers, ApiVersion};
use crate::routes::Path;
use crate::routes::Query;
use crate::{Error, Result};
use axum::Json;
use nexus_common::models::post::PostRelationships;
use nexus_common::models::tag::post::TagPost;
use nexus_common::models::tag::TagDetails;
use serde::Deserialize;
use tracing::debug;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct PostPath {
    pub author_id: PubkyId,
    pub post_id: PostId,
}

/// The query of `GET /post/{author_id}/{post_id}`, with `include_attachment_metadata` as
/// version `V` reads it: a flag on `/v0`, nothing on `/v1`, which has no file details. Every
/// other parameter is the same on both.
#[derive(Default, Deserialize, Debug)]
#[serde(bound(deserialize = ""))]
pub struct PostViewQuery<V: ApiVersion> {
    pub viewer_id: Option<PubkyId>,
    pub limit_tags: Option<BoundedLimit<5, 100>>,
    pub limit_taggers: Option<BoundedLimit<5, 100>>,
    #[serde(default)]
    pub include_attachment_metadata: V::IncludeAttachmentMetadata,
}

/// The one body behind `post_view_handler_v0` and `post_view_handler_v1`: the view in the
/// version's shape.
pub async fn post_view<V: ApiVersion>(
    Path(PostPath { author_id, post_id }): Path<PostPath>,
    Query(query): Query<PostViewQuery<V>>,
) -> Result<Json<V::PostView>> {
    debug!(
        "GET {POST_ROUTE} author_id:{}, post_id:{}, viewer_id:{:?}, limit_tags:{:?}, limit_taggers:{:?}",
        author_id,
        post_id,
        query.viewer_id,
        query.limit_tags,
        query.limit_taggers
    );
    // Avoid by default WoT tags in a Post. We could add as `depth` argument for that specific use case
    match PostViewDetailed::get_by_id(
        &author_id,
        &post_id,
        query.viewer_id.as_deref(),
        query.limit_tags.map(|l| l.value()),
        query.limit_taggers.map(|l| l.value()),
        query.include_attachment_metadata.into(),
    )
    .await?
    {
        Some(post) => Ok(Json(V::PostView::from(post))),
        None => Err(Error::post_not_found(author_id, post_id)),
    }
}

versioned_handlers! {
    post_view_handler_v0 / post_view_handler_v1 => post_view(path: Path<PostPath>, query: Query<shapes::PostViewQuery>) -> shapes::PostView;
    #[utoipa::path(
        get,
        path = POST_ROUTE,
        operation_id = "post_view_handler",
        description = "Post view",
        tag = "Post",
        params(
            ("author_id" = PubkyId, Path, description = "Author Pubky ID"),
            ("post_id" = PostId, Path, description = "Post Crockford32 ID"),
            ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID"),
            ("limit_tags" = Option<BoundedLimit<5, 100>>, Query, description = "Upper limit on the number of tags for the post (1–100, **default** 5)"),
            ("limit_taggers" = Option<BoundedLimit<5, 100>>, Query, description = "Upper limit on the number of taggers per tag (1–100, **default** 5)"),
            shapes::IncludeAttachmentMetadata,
        )
    )]
    responses(
        (status = 200, description = "Post"),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "Post not found"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
}

/// The schemas the post view nests in every version; the view itself is each version's.
#[derive(OpenApi)]
#[openapi(components(schemas(PostRelationships, TagPost, TagDetails, PubkyId, PostId)))]
pub struct PostViewApiDoc;
