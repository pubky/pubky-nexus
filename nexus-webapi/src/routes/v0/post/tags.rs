use crate::models::{BoundedLimit, BoundedSkip, PostId, PubkyId, TagLabel};
use crate::routes::v0::endpoints::{POST_TAGGERS_ROUTE, POST_TAGS_ROUTE};
use crate::routes::v0::post::view::PostPath;
use crate::routes::v0::types::resolve_tag_wot_depth;
use crate::routes::v0::user::tags::TaggersQuery;
use crate::routes::v0::{TaggersInfoResponse, TagsQuery};
use crate::routes::Path;
use crate::routes::Query;
use crate::{Error, Result};
use axum::Json;
use nexus_common::models::tag::post::TagPost;
use nexus_common::models::tag::traits::{TagCollection, TaggersCollection};
use nexus_common::models::tag::TagDetails;
use serde::Deserialize;
use tracing::debug;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct PostTaggersPath {
    pub author_id: PubkyId,
    pub post_id: PostId,
    pub label: TagLabel,
}

#[utoipa::path(
    get,
    path = POST_TAGS_ROUTE,
    description = "Post tags",
    tag = "Post",
    params(
        ("author_id" = PubkyId, Path, description = "Author Pubky ID"),
        ("post_id" = PostId, Path, description = "Post ID"),
        ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID"),
        ("skip_tags" = Option<BoundedSkip<10_000>>, Query, description = "Skip N tags (0–10 000, **default** 0)"),
        ("limit_tags" = Option<BoundedLimit<5, 100>>, Query, description = "Upper limit on the number of tags (1–100, **default** 5 in the global view; the WoT view returns the full trusted set by default—pass `limit_tags` for a compact view)"),
        ("limit_taggers" = Option<BoundedLimit<5, 100>>, Query, description = "Upper limit on the number of taggers per tag (1–100, **default** 5)"),
        ("depth" = Option<u8>, Query, description = "WoT depth (1-3). Provide it together with `viewer_id` to filter post tags through the viewer's Web of Trust. In the WoT view `limit_tags` defaults to the full trusted set so a trusted moderation tag is not paginated out by tagger count (pass `limit_tags` for a compact view); `skip_tags`/`limit_taggers` apply as in the global view. `viewer_id` without `depth` returns the global tag view; `depth` without `viewer_id`, or an out-of-range `depth` with `viewer_id`, is rejected with 400."),
    ),
    responses(
        (status = 400, description = "Invalid input"),
        (status = 404, description = "Post not found"),
        (status = 200, description = "Post tags", body = Vec<TagDetails>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn post_tags_handler(
    Path(PostPath { author_id, post_id }): Path<PostPath>,
    Query(query): Query<TagsQuery>,
) -> Result<Json<Vec<TagDetails>>> {
    debug!(
        "GET {POST_TAGS_ROUTE} author_id:{}, post_id: {}, skip_tags:{:?}, limit_tags:{:?}, limit_taggers:{:?}, depth:{:?}",
        author_id, post_id, query.limit_tags, query.skip_tags, query.limit_taggers, query.depth
    );
    let wot_depth = resolve_tag_wot_depth(query.viewer_id.as_deref(), query.depth)?;
    let tags = match (query.viewer_id.as_deref(), wot_depth) {
        // WoT-filtered post tags (graph-only): tags from taggers in the viewer's Web of Trust.
        (Some(viewer_id), Some(depth)) => {
            TagPost::get_wot_tags_by_post(
                &author_id,
                &post_id,
                viewer_id,
                depth,
                query.skip_tags.map(|s| s.value()),
                query.limit_tags.map(|l| l.value()),
                query.limit_taggers.map(|l| l.value()),
            )
            .await?
        }
        _ => {
            TagPost::get_by_id(
                &author_id,
                Some(&post_id),
                query.skip_tags.map(|s| s.value()),
                query.limit_tags.map(|l| l.value()),
                query.limit_taggers.map(|l| l.value()),
                query.viewer_id.as_deref(),
                None,
            )
            .await?
        }
    };
    match tags {
        Some(tags) => Ok(Json(tags)),
        None => Err(Error::post_not_found(author_id, post_id)),
    }
}

#[utoipa::path(
    get,
    path = POST_TAGGERS_ROUTE,
    description = "Post specific label Taggers",
    tag = "Post",
    params(
        ("author_id" = PubkyId, Path, description = "Author Pubky ID"),
        ("label" = TagLabel, Path, description = "Tag name"),
        ("post_id" = PostId, Path, description = "Post ID"),
        ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID"),
        ("depth" = Option<u8>, Query, description = "Ignored: Web of Trust filtering is not applied to taggers"),
        ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Skip N taggers (0–10 000, **default** 0)"),
        ("limit" = Option<BoundedLimit<40, 100>>, Query, description = "Number of taggers to return (1–100, **default** 40)")
    ),
    responses(
        (status = 200, description = "Post tags", body = TaggersInfoResponse),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn post_taggers_handler(
    Path(PostTaggersPath {
        author_id,
        post_id,
        label,
    }): Path<PostTaggersPath>,
    Query(taggers_query): Query<TaggersQuery>,
) -> Result<Json<TaggersInfoResponse>> {
    debug!(
        "GET {POST_TAGGERS_ROUTE} author_id:{}, post_id: {}, label: {}, viewer_id:{:?}, skip:{}, limit:{}",
        author_id, post_id, label, taggers_query.tags_query.viewer_id,
        taggers_query.pagination.skip_value(), taggers_query.pagination.limit_value()
    );

    let pagination = taggers_query.pagination.to_pagination(None, None);

    let taggers = TagPost::get_tagger_by_id(
        &author_id,
        Some(&post_id),
        &label,
        pagination,
        taggers_query.tags_query.viewer_id.as_deref(),
        None,
    )
    .await?;
    Ok(Json(TaggersInfoResponse::from(taggers)))
}

#[derive(OpenApi)]
#[openapi(
    paths(post_tags_handler, post_taggers_handler),
    components(schemas(TagDetails, TaggersInfoResponse, PubkyId, PostId, TagLabel))
)]
pub struct PostTagsApiDoc;
