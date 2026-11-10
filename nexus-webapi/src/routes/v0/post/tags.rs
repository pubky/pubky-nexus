use crate::models::{BoundedLimit, BoundedSkip, PostId, PubkyId};
use crate::routes::v0::endpoints::{POST_TAGGERS_ROUTE, POST_TAGS_ROUTE};
use crate::routes::v0::post::view::PostPath;
use crate::routes::v0::types::resolve_tag_wot_depth;
use crate::routes::v0::user::tags::TaggersQuery;
use crate::routes::v0::version::{versioned_handlers, ApiVersion};
use crate::routes::v0::TagsQuery;
use crate::routes::Path;
use crate::routes::Query;
use crate::{Error, Result};
use axum::Json;
use nexus_common::models::tag::post::TagPost;
use nexus_common::models::tag::traits::{TagCollection, TaggersCollection};
use serde::Deserialize;
use tracing::debug;
use utoipa::OpenApi;

/// The path of `GET /post/{author_id}/{post_id}/taggers/{label}`, with `label` folded as
/// version `V` folds it.
#[derive(Deserialize)]
#[serde(bound(deserialize = ""))]
pub struct PostTaggersPath<V: ApiVersion> {
    pub author_id: PubkyId,
    pub post_id: PostId,
    pub label: V::TagLabel,
}

/// The one body behind `post_tags_handler_v0` and `post_tags_handler_v1`: the tags in the
/// version's shape.
pub async fn post_tags<V: ApiVersion>(
    Path(PostPath { author_id, post_id }): Path<PostPath>,
    Query(query): Query<TagsQuery>,
) -> Result<Json<Vec<V::TagDetails>>> {
    debug!(
        "GET {POST_TAGS_ROUTE} author_id:{}, post_id: {}, skip_tags:{:?}, limit_tags:{:?}, limit_taggers:{:?}, depth:{:?}",
        author_id, post_id, query.skip_tags, query.limit_tags, query.limit_taggers, query.depth
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
                query.skip_tags_as_usize(),
                query.limit_tags_as_usize(),
                query.limit_taggers_as_usize(),
            )
            .await?
        }
        _ => {
            TagPost::get_by_id(
                &author_id,
                Some(&post_id),
                query.skip_tags_as_usize(),
                query.limit_tags_as_usize(),
                query.limit_taggers_as_usize(),
                query.viewer_id.as_deref(),
                None,
            )
            .await?
        }
    };
    match tags {
        Some(tags) => Ok(Json(tags.into_iter().map(V::TagDetails::from).collect())),
        None => Err(Error::post_not_found(author_id, post_id)),
    }
}

versioned_handlers! {
    post_tags_handler_v0 / post_tags_handler_v1 => post_tags(path: Path<PostPath>, query: Query<TagsQuery>) -> Vec<shapes::TagDetails>;
    #[utoipa::path(
        get,
        path = POST_TAGS_ROUTE,
        operation_id = "post_tags_handler",
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
        )
    )]
    responses(
        (status = 200, description = "Post tags"),
        (status = 400, description = "Invalid input"),
        (status = 404, description = "Post not found"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
}

/// The one body behind `post_taggers_handler_v0` and `post_taggers_handler_v1`: the taggers in
/// the version's shape, of the label as the version folds it.
pub async fn post_taggers<V: ApiVersion>(
    Path(PostTaggersPath {
        author_id,
        post_id,
        label,
    }): Path<PostTaggersPath<V>>,
    Query(taggers_query): Query<TaggersQuery>,
) -> Result<Json<V::TaggersInfoResponse>> {
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
    Ok(Json(V::TaggersInfoResponse::from(taggers)))
}

versioned_handlers! {
    post_taggers_handler_v0 / post_taggers_handler_v1 => post_taggers(path: Path<shapes::PostTaggersPath>, query: Query<TaggersQuery>) -> shapes::TaggersInfoResponse;
    #[utoipa::path(
        get,
        path = POST_TAGGERS_ROUTE,
        operation_id = "post_taggers_handler",
        description = "Post specific label Taggers",
        tag = "Post",
        params(
            ("author_id" = PubkyId, Path, description = "Author Pubky ID"),
            ("label" = shapes::TagLabel, Path, description = "Tag name"),
            ("post_id" = PostId, Path, description = "Post ID"),
            ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID"),
            ("depth" = Option<u8>, Query, description = "Ignored: Web of Trust filtering is not applied to taggers"),
            ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Skip N taggers (0–10 000, **default** 0)"),
            ("limit" = Option<BoundedLimit<40, 100>>, Query, description = "Number of taggers to return (1–100, **default** 40)")
        )
    )]
    responses(
        (status = 200, description = "Post tags"),
        (status = 400, description = "Invalid parameters"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
}

/// The schemas the post tag routes share; the routes themselves, whose tags, taggers and label
/// are the version's, come with the version's document.
#[derive(OpenApi)]
#[openapi(components(schemas(PubkyId, PostId)))]
pub struct PostTagsApiDoc;
