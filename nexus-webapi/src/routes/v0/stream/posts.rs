use crate::models::{GlobalPostId, GlobalPostIds, PostId, PostStreamDetailed, PubkyId, Tags};
use crate::routes::v0::endpoints::{
    STREAM_POSTS_BY_IDS_ROUTE, STREAM_POSTS_ROUTE, STREAM_POST_KEYS_ROUTE,
};
use crate::routes::v0::types::parse_string_to_u8;
use crate::routes::Json as RequestJson;
use crate::routes::Query;
use crate::{Error, Result as AppResult};
use axum::Json;
use nexus_common::db::kv::SortOrder;
use nexus_common::types::StreamSorting;
use nexus_common::{
    models::post::{PostKeyStream, PostStream, StreamSource},
    types::{Pagination, WotDepth},
};
use pubky_app_specs::PubkyAppPostKind;
use serde::Deserialize;
use tracing::debug;
use utoipa::{OpenApi, ToSchema};

/// Discriminant for the source of posts in a stream.
/// Mirrors the variant structure of StreamSource but does not carry ID payloads,
/// since the actual IDs are validated at the API boundary via dedicated query params.
#[derive(Deserialize, Debug, ToSchema, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StreamSourceKind {
    PostReplies,
    Following,
    Followers,
    Friends,
    Bookmarks,
    Author,
    AuthorReplies,
    Collection,
    Wot,
    WotDomain,
    #[default]
    All,
}

/// Validates the requested WoT depth, falling back to the default when absent.
fn resolve_wot_depth(depth: Option<u8>) -> AppResult<WotDepth> {
    match depth {
        Some(depth) => WotDepth::new(depth).map_err(Error::invalid_input),
        None => Ok(WotDepth::default()),
    }
}

/// Convert a validated query into the internal StreamSource used by nexus-common.
/// Returns an error if the selected `StreamSourceKind` requires fields that are missing
fn build_stream_source(
    kind: &StreamSourceKind,
    author_id: Option<&PubkyId>,
    observer_id: Option<&PubkyId>,
    post_id: Option<&PostId>,
    depth: Option<u8>,
    domain_tags: Option<&Tags>,
) -> AppResult<StreamSource> {
    match kind {
        StreamSourceKind::PostReplies => match (post_id, author_id) {
            (Some(post_id), Some(author_id)) => Ok(StreamSource::PostReplies {
                post_id: post_id.to_string(),
                author_id: author_id.to_string(),
            }),
            _ => Err(Error::invalid_input(
                "source 'post_replies' requires both 'post_id' and 'author_id' parameters",
            )),
        },
        StreamSourceKind::Following => match observer_id {
            Some(observer_id) => Ok(StreamSource::Following {
                observer_id: observer_id.to_string(),
            }),
            None => Err(Error::invalid_input(
                "source 'following' requires 'observer_id' parameter",
            )),
        },
        StreamSourceKind::Followers => match observer_id {
            Some(observer_id) => Ok(StreamSource::Followers {
                observer_id: observer_id.to_string(),
            }),
            None => Err(Error::invalid_input(
                "source 'followers' requires 'observer_id' parameter",
            )),
        },
        StreamSourceKind::Friends => match observer_id {
            Some(observer_id) => Ok(StreamSource::Friends {
                observer_id: observer_id.to_string(),
            }),
            None => Err(Error::invalid_input(
                "source 'friends' requires 'observer_id' parameter",
            )),
        },
        StreamSourceKind::Bookmarks => match observer_id {
            Some(observer_id) => Ok(StreamSource::Bookmarks {
                observer_id: observer_id.to_string(),
            }),
            None => Err(Error::invalid_input(
                "source 'bookmarks' requires 'observer_id' parameter",
            )),
        },
        StreamSourceKind::Author => match author_id {
            Some(author_id) => Ok(StreamSource::Author {
                author_id: author_id.to_string(),
            }),
            None => Err(Error::invalid_input(
                "source 'author' requires 'author_id' parameter",
            )),
        },
        StreamSourceKind::AuthorReplies => match author_id {
            Some(author_id) => Ok(StreamSource::AuthorReplies {
                author_id: author_id.to_string(),
            }),
            None => Err(Error::invalid_input(
                "source 'author_replies' requires 'author_id' parameter",
            )),
        },
        StreamSourceKind::Collection => match (author_id, post_id) {
            (Some(author_id), Some(post_id)) => Ok(StreamSource::Collection {
                author_id: author_id.to_string(),
                post_id: post_id.to_string(),
            }),
            (None, _) => Err(Error::invalid_input(
                "source 'collection' requires 'author_id' parameter",
            )),
            (_, None) => Err(Error::invalid_input(
                "source 'collection' requires 'post_id' parameter",
            )),
        },
        StreamSourceKind::Wot => match observer_id {
            Some(observer_id) => Ok(StreamSource::Wot {
                observer_id: observer_id.to_string(),
                depth: resolve_wot_depth(depth)?,
            }),
            None => Err(Error::invalid_input(
                "source 'wot' requires 'observer_id' parameter",
            )),
        },
        StreamSourceKind::WotDomain => match (observer_id, domain_tags) {
            (Some(observer_id), Some(domain_tags)) => Ok(StreamSource::WotDomain {
                observer_id: observer_id.to_string(),
                depth: resolve_wot_depth(depth)?,
                domain_tags: domain_tags.to_string_vec(),
            }),
            (None, _) => Err(Error::invalid_input(
                "source 'wot_domain' requires 'observer_id' parameter",
            )),
            (_, None) => Err(Error::invalid_input(
                "source 'wot_domain' requires 'domain_tags' parameter",
            )),
        },
        StreamSourceKind::All => Ok(StreamSource::All),
    }
}

#[derive(Deserialize, Debug, ToSchema)]
pub struct PostStreamQuery {
    #[serde(default)]
    pub source: StreamSourceKind,
    pub author_id: Option<PubkyId>,
    pub observer_id: Option<PubkyId>,
    pub post_id: Option<PostId>,
    #[serde(flatten)]
    pub pagination: Pagination,
    pub order: Option<SortOrder>,
    pub sorting: Option<StreamSorting>,
    pub viewer_id: Option<PubkyId>,
    pub tags: Option<Tags>,
    #[serde(default, deserialize_with = "parse_string_to_u8")]
    pub depth: Option<u8>,
    pub domain_tags: Option<Tags>,
    pub kind: Option<PubkyAppPostKind>,
    #[serde(default)]
    pub include_attachment_metadata: bool,
}

impl PostStreamQuery {
    pub fn initialize_defaults(&mut self) {
        self.pagination.skip.get_or_insert(0);
        self.pagination.limit = Some(self.pagination.limit.unwrap_or(10).min(30));
        self.sorting.get_or_insert(StreamSorting::Timeline);
    }

    pub fn build_source(&self) -> AppResult<StreamSource> {
        build_stream_source(
            &self.source,
            self.author_id.as_ref(),
            self.observer_id.as_ref(),
            self.post_id.as_ref(),
            self.depth,
            self.domain_tags.as_ref(),
        )
    }

    pub fn extract_stream_params(&self) -> AppResult<(StreamSource, StreamSorting, SortOrder)> {
        Ok((
            self.build_source()?,
            self.sorting.clone().unwrap_or_default(),
            self.order.clone().unwrap_or_default(),
        ))
    }

    /// Must run before `initialize_defaults()` — otherwise `sorting` would
    /// always read as `Some` and reject every collection request.
    pub fn validate_source_compat(&self) -> AppResult<()> {
        if !matches!(self.source, StreamSourceKind::Collection) {
            return Ok(());
        }
        let incompatible = [
            ("tags", self.tags.is_some()),
            ("kind", self.kind.is_some()),
            ("sorting", self.sorting.is_some()),
            ("order", self.order.is_some()),
            ("start", self.pagination.start.is_some()),
            ("end", self.pagination.end.is_some()),
        ];
        if let Some((name, _)) = incompatible.iter().find(|(_, present)| *present) {
            return Err(Error::invalid_input(format!(
                "`{name}` is not supported with `source=collection`"
            )));
        }
        Ok(())
    }
}

#[utoipa::path(
    get,
    path = STREAM_POSTS_ROUTE,
    tag = "Stream",
    params(
        ("source" = Option<StreamSourceKind>, Query, description = "Source of posts for streams with viewer (following, followers, friends, bookmarks, post_replies, author, author_replies, collection, wot, wot_domain, all). For `source=collection`: provide `author_id` + `post_id` of the Collection post; items are returned in curator order. `tags`, `kind`, `sorting`, `order`, `start`, `end` are all rejected with 400 (incompatible with the curator-ordered result set). Items whose underlying post is missing (deleted, not indexed) or whose URI is malformed/non-post are dropped during hydration; pages may be shorter than `limit`. Pagination via `skip`/`limit` is not stable across deletions, if an item is removed between page fetches, the same `skip` returns a different window. The FE can identify dropped items by diffing the response against the Collection envelope's `items[]`."),
        ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID"),
        ("observer_id" = Option<PubkyId>, Query, description = "Observer Pubky ID. The central point for streams with Reach"),
        ("author_id" = Option<PubkyId>, Query, description = "Filter posts by an specific author User ID"),
        ("post_id" = Option<PostId>, Query, description = "This parameter is needed when we want to retrieve the replies stream for a post"),
        ("sorting" = Option<StreamSorting>, Query, description = "Sort method (`timeline` or `total_engagement`). Ties are broken by post id; pagination across equal scores is best-effort."),
        ("order" = Option<SortOrder>, Query, description = "Ordering of response list. Either 'ascending' or 'descending'. Defaults to descending."),
        ("tags" = Option<Tags>, Query, description = "Filter by a list of comma-separated tags (max 5). E.g.,`&tags=dev,free,opensource`. Only posts matching at least one of the tags will be returned."),
        ("depth" = Option<u8>, Query, description = "WoT traversal depth (1-3, default 2) for `source=wot` / `source=wot_domain`. Ignored for other sources."),
        ("domain_tags" = Option<Tags>, Query, description = "Required for `source=wot_domain`. Comma-separated tag labels (max 5); returns posts by authors tagged with any of these by the observer's WoT. E.g. `&domain_tags=bitcoiner,btc-dev`. Ignored for other sources."),
        ("kind" = Option<PubkyAppPostKind>, Query, description = "Filter by post kind: short, long, image, video, link, file, collection."),
        ("skip" = Option<usize>, Query, description = "Skip N posts"),
        ("limit" = Option<usize>, Query, description = "Retrieve N posts"),
        ("start" = Option<usize>, Query, description = "The start of the stream timeframe or score. Posts with a timestamp/score greater than this value will be excluded from the results"),
        ("end" = Option<usize>, Query, description = "The end of the stream timeframe or score. Posts with a timestamp/score less than this value will be excluded from the results"),
        ("include_attachment_metadata" = Option<bool>, Query, description = "Include file metadata for post attachments"),
    ),
    responses(
        (status = 200, description = "Posts stream", body = PostStreamDetailed),
        (status = 500, description = "Internal server error")
    ),
    description = r#"Stream Posts: Retrieve a stream of posts.


The `source` parameter determines the type of stream. Depending on the `source`, certain parameters are required:
- *following*, *followers*, *friends*, *bookmarks*: Requires **observer_id**.
- *post_replies*: Requires **author_id** and **post_id** to filter replies to a specific post.
- *author*:  Requires  **author_id** to filter posts by a specific author.
- *author_replies*:  Requires  **author_id** to filter replies by a specific author.
- *collection*: Requires **author_id** and **post_id** of the Collection post; items are returned in curator order.

- *wot*: Requires **observer_id**. Posts from users in the observer's Web of Trust (transitive follows, `depth` 1-3, default 2).
- *wot_domain*: Requires **observer_id** and **domain_tags**. Posts from users whom the observer's Web of Trust has tagged with any of `domain_tags`.

Ensure that you provide the necessary parameters based on the selected `source`. If a required parameter is missing, a 400 Bad Request error will be returned."#
)]
pub async fn stream_posts_handler(
    Query(mut query): Query<PostStreamQuery>,
) -> AppResult<Json<PostStreamDetailed>> {
    debug!("GET {STREAM_POSTS_ROUTE}");

    query.validate_source_compat()?; // before initialize_defaults
    query.initialize_defaults();
    let (source, sorting, order) = query.extract_stream_params()?;
    let include_attachment_metadata = query.include_attachment_metadata;
    let tags = query.tags.as_ref().map(Tags::to_string_vec);

    match PostStream::get_posts(
        source,
        query.pagination,
        order,
        sorting,
        query.viewer_id.as_deref(),
        tags,
        query.kind,
    )
    .await?
    {
        Some(stream) => Ok(Json(
            PostStreamDetailed::from_post_views(stream.0, include_attachment_metadata).await?,
        )),
        None => Ok(Json(PostStreamDetailed::default())),
    }
}

#[utoipa::path(
    get,
    path = STREAM_POST_KEYS_ROUTE,
    tag = "Stream",
    params(
        ("source" = Option<StreamSourceKind>, Query, description = "Source of posts for streams with viewer (following, followers, friends, bookmarks, post_replies, author, author_replies, collection, wot, wot_domain, all). For `source=collection`: provide `author_id` + `post_id` of the Collection post; keys are returned in curator order. `tags`, `kind`, `sorting`, `order`, `start`, `end` are all rejected with 400 (incompatible with the curator-ordered result set). Like every other source, the returned keys are a best-effort snapshot, they may reference posts that have since been deleted or are not yet indexed; callers should hydrate via `GET /v0/stream/posts?source=collection&author_id=...&post_id=...` (or `POST /v0/stream/posts/by_ids`) which drops unresolved refs. Pagination via `skip`/`limit` is not stable across deletions."),
        ("observer_id" = Option<PubkyId>, Query, description = "Observer Pubky ID. The central point for streams with Reach"),
        ("author_id" = Option<PubkyId>, Query, description = "Filter posts by an specific author User ID"),
        ("post_id" = Option<PostId>, Query, description = "This parameter is needed when we want to retrieve the replies stream for a post"),
        ("sorting" = Option<StreamSorting>, Query, description = "Sort method (`timeline` or `total_engagement`). Ties are broken by post id; pagination across equal scores is best-effort."),
        ("order" = Option<SortOrder>, Query, description = "Ordering of response list. Either 'ascending' or 'descending'. Defaults to descending."),
        ("tags" = Option<Tags>, Query, description = "Filter by a list of comma-separated tags (max 5). E.g.,`&tags=dev,free,opensource`. Only posts matching at least one of the tags will be returned."),
        ("depth" = Option<u8>, Query, description = "WoT traversal depth (1-3, default 2) for `source=wot` / `source=wot_domain`. Ignored for other sources."),
        ("domain_tags" = Option<Tags>, Query, description = "Required for `source=wot_domain`. Comma-separated tag labels (max 5); returns posts by authors tagged with any of these by the observer's WoT. E.g. `&domain_tags=bitcoiner,btc-dev`. Ignored for other sources."),
        ("kind" = Option<PubkyAppPostKind>, Query, description = "Filter by post kind: short, long, image, video, link, file, collection."),
        ("skip" = Option<usize>, Query, description = "Skip N posts"),
        ("limit" = Option<usize>, Query, description = "Retrieve N posts"),
        ("start" = Option<usize>, Query, description = "The start of the stream timeframe or score. Posts with a timestamp/score greater than this value will be excluded from the results"),
        ("end" = Option<usize>, Query, description = "The end of the stream timeframe or score. Posts with a timestamp/score less than this value will be excluded from the results"),
    ),
    responses(
        (status = 200, description = "Post key stream", body = PostKeyStream),
        (status = 500, description = "Internal server error")
    ),
    description = r#"Stream Post Keys: Retrieve a stream of post keys

The `source` parameter determines the type of stream. Depending on the `source`, certain parameters are required:
- *following*, *followers*, *friends*, *bookmarks*: Requires **observer_id**.
- *post_replies*: Requires **author_id** and **post_id** to filter replies to a specific post.
- *author*:  Requires  **author_id** to filter posts by a specific author.
- *author_replies*:  Requires  **author_id** to filter replies by a specific author.
- *collection*: Requires **author_id** and **post_id** of the Collection post; keys are returned in curator order.

- *wot*: Requires **observer_id**. Posts from users in the observer's Web of Trust (transitive follows, `depth` 1-3, default 2).
- *wot_domain*: Requires **observer_id** and **domain_tags**. Posts from users whom the observer's Web of Trust has tagged with any of `domain_tags`.

Ensure that you provide the necessary parameters based on the selected `source`. If a required parameter is missing, a 400 Bad Request error will be returned."#
)]
pub async fn stream_post_keys_handler(
    Query(mut query): Query<PostStreamQuery>,
) -> AppResult<Json<PostKeyStream>> {
    debug!("GET {STREAM_POST_KEYS_ROUTE}");

    query.validate_source_compat()?; // before initialize_defaults
    query.initialize_defaults();
    let (source, sorting, order) = query.extract_stream_params()?;
    let tags = query.tags.as_ref().map(Tags::to_string_vec);

    match PostStream::get_post_keys(source, query.pagination, order, sorting, tags, query.kind)
        .await?
    {
        Some(stream) => Ok(Json(stream)),
        None => Ok(Json(PostKeyStream::default())),
    }
}

#[derive(ToSchema, Deserialize)]
pub struct PostStreamByIdsRequest {
    pub post_ids: GlobalPostIds,
    pub viewer_id: Option<PubkyId>,
    #[serde(default)]
    pub include_attachment_metadata: bool,
}
#[utoipa::path(
    post,
    path = STREAM_POSTS_BY_IDS_ROUTE,
    tag = "Stream",
    description = "Stream post by ID. This is a POST request because we're passing a potentially large list of post IDs in the request body",
    request_body = PostStreamByIdsRequest,
    responses(
        (status = 200, description = "Post stream", body = PostStreamDetailed),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_posts_by_ids_handler(
    RequestJson(request): RequestJson<PostStreamByIdsRequest>,
) -> AppResult<Json<PostStreamDetailed>> {
    debug!(
        "POST {} post_ids size {:?}",
        STREAM_POSTS_BY_IDS_ROUTE,
        request.post_ids.0.len()
    );

    let post_ids = request.post_ids.into_string_vec();

    match PostStream::from_listed_post_ids(request.viewer_id.as_deref(), &post_ids).await? {
        Some(stream) => Ok(Json(
            PostStreamDetailed::from_post_views(stream.0, request.include_attachment_metadata)
                .await?,
        )),
        None => Ok(Json(PostStreamDetailed::default())),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        stream_posts_handler,
        stream_post_keys_handler,
        stream_posts_by_ids_handler
    ),
    components(schemas(
        PostKeyStream,
        PostStreamDetailed,
        StreamSorting,
        StreamSourceKind,
        SortOrder,
        PubkyId,
        PostId,
        Tags,
        GlobalPostId
    ))
)]
pub struct StreamPostsApiDocs;
