use crate::models::{
    BoundedLimit, BoundedPagination, BoundedSkip, GlobalPostId, GlobalPostIds, PostId,
    PostStreamDetailed, PubkyId, Tags,
};
use crate::routes::v0::endpoints::{
    STREAM_POSTS_BY_IDS_ROUTE, STREAM_POSTS_ROUTE, STREAM_POST_KEYS_ROUTE,
};
use crate::routes::v0::types::parse_string_to_u8;
use crate::routes::v0::version::{versioned_handlers, ApiVersion};
use crate::routes::Json as RequestJson;
use crate::routes::Query;
use crate::{Error, Result as AppResult};
use axum::Json;
use const_format::concatcp;
use nexus_common::db::kv::SortOrder;
use nexus_common::types::StreamSorting;
use nexus_common::{
    models::post::{KindFilter, PostKeyStream, PostStream, StreamSource},
    types::{DomainTrust, WotDepth},
};
use serde::Deserialize;
use tracing::debug;
use utoipa::{OpenApi, ToSchema};

/// Discriminant for the source of posts in a stream.
/// Mirrors the variant structure of StreamSource but does not carry ID payloads,
/// since the actual IDs are validated at the API boundary via dedicated query params.
// This is `/v0`'s reading of `source=` and the one the body builds from; `/v1` reads
// `v1::StreamSourceKind`, the same names without `bookmarks`, and converts into this. A
// line comment: a doc comment would be the `/v0` schema's description, which is frozen.
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
    PostCollections,
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

/// Resolves the `wot_domain` trust set from `depth`. `depth=0` selects the
/// observer-only ("Me") set; absent falls back to the default network reach.
/// Unlike `source=wot`, `depth=0` is valid here and means "no follow traversal".
fn resolve_domain_trust(depth: Option<u8>) -> AppResult<DomainTrust> {
    match depth {
        None => Ok(DomainTrust::Network(WotDepth::default())),
        Some(0) => Ok(DomainTrust::Me),
        Some(depth) => WotDepth::new(depth)
            .map(DomainTrust::Network)
            .map_err(Error::invalid_input),
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
        StreamSourceKind::PostCollections => match (author_id, post_id) {
            (Some(author_id), Some(post_id)) => Ok(StreamSource::PostCollections {
                author_id: author_id.to_string(),
                post_id: post_id.to_string(),
            }),
            (None, _) => Err(Error::invalid_input(
                "source 'post_collections' requires 'author_id' parameter",
            )),
            (_, None) => Err(Error::invalid_input(
                "source 'post_collections' requires 'post_id' parameter",
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
                trust: resolve_domain_trust(depth)?,
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

/// The query of `GET /stream/posts` and `GET /stream/posts/keys`, with `source` in version
/// `V`'s names (`bookmarks` is `/v0`'s alone) and `kind` and `exclude_kinds` in its kind names:
/// the stored ones on `/v0`, the `social/v1` ones on `/v1`. Every other parameter is the same
/// on both.
#[derive(Deserialize, Debug)]
#[serde(bound(deserialize = ""))]
pub struct PostStreamQuery<V: ApiVersion> {
    #[serde(default)]
    pub source: V::StreamSource,
    pub author_id: Option<PubkyId>,
    pub observer_id: Option<PubkyId>,
    pub post_id: Option<PostId>,
    #[serde(flatten)]
    pub pagination: BoundedPagination<10_000, 10, 50>,
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub order: Option<SortOrder>,
    pub sorting: Option<StreamSorting>,
    pub viewer_id: Option<PubkyId>,
    pub tags: Option<Tags>,
    #[serde(default, deserialize_with = "parse_string_to_u8")]
    pub depth: Option<u8>,
    pub domain_tags: Option<Tags>,
    pub kind: Option<V::PostKind>,
    pub exclude_kinds: Option<V::PostKinds>,
    #[serde(default)]
    pub locked: bool,
    /// A flag on `/v0`, nothing on `/v1`, which has no file details.
    #[serde(default)]
    pub include_attachment_metadata: V::IncludeAttachmentMetadata,
}

impl<V: ApiVersion> PostStreamQuery<V> {
    pub fn initialize_defaults(&mut self) {
        self.sorting.get_or_insert(StreamSorting::Timeline);
    }

    /// The source as the shared builder reads it, whichever version's names it was parsed in.
    pub fn source(&self) -> StreamSourceKind {
        self.source.clone().into()
    }

    pub fn build_source(&self) -> AppResult<StreamSource> {
        build_stream_source(
            &self.source(),
            self.author_id.as_ref(),
            self.observer_id.as_ref(),
            self.post_id.as_ref(),
            self.depth,
            self.domain_tags.as_ref(),
        )
    }

    /// Maps the mutually exclusive `kind` / `exclude_kinds` / `locked` params
    /// (enforced by `validate_source_compat`) to the internal filter, in the stored names.
    pub fn kind_filter(&self) -> Option<KindFilter> {
        if self.locked {
            return Some(KindFilter::Locked);
        }
        match (&self.kind, &self.exclude_kinds) {
            (Some(kind), _) => Some(KindFilter::Kind(kind.clone().into())),
            (None, Some(exclude)) => Some(KindFilter::Exclude(exclude.clone().into())),
            (None, None) => None,
        }
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
        if self.kind.is_some() && self.exclude_kinds.is_some() {
            return Err(Error::invalid_input(
                "`kind` and `exclude_kinds` are mutually exclusive",
            ));
        }
        if self.locked && (self.kind.is_some() || self.exclude_kinds.is_some()) {
            return Err(Error::invalid_input(
                "`locked` cannot be combined with `kind` or `exclude_kinds`",
            ));
        }
        let source = self.source();
        // Reply streams are served by dedicated sorted sets; the Cypher
        // fallback that kind filters route to has no reply MATCH arm and
        // would silently return the author's parent posts instead.
        if matches!(
            source,
            StreamSourceKind::PostReplies | StreamSourceKind::AuthorReplies
        ) && self.kind_filter().is_some()
        {
            return Err(Error::invalid_input(
                "`kind`, `exclude_kinds` and `locked` are not supported with `source=post_replies` or `source=author_replies`",
            ));
        }
        // Every result is a Collection, so a kind filter can only no-op or
        // silently empty the page.
        if matches!(source, StreamSourceKind::PostCollections) && self.kind_filter().is_some() {
            return Err(Error::invalid_input(
                "`kind`, `exclude_kinds` and `locked` are not supported with `source=post_collections`",
            ));
        }
        if !matches!(source, StreamSourceKind::Collection) {
            return Ok(());
        }
        let incompatible = [
            ("tags", self.tags.is_some()),
            ("kind", self.kind.is_some()),
            ("exclude_kinds", self.exclude_kinds.is_some()),
            ("locked", self.locked),
            ("sorting", self.sorting.is_some()),
            ("order", self.order.is_some()),
            ("start", self.start.is_some()),
            ("end", self.end.is_some()),
        ];
        if let Some((name, _)) = incompatible.iter().find(|(_, present)| *present) {
            return Err(Error::invalid_input(format!(
                "`{name}` is not supported with `source=collection`"
            )));
        }
        Ok(())
    }
}

/// The one body behind `stream_posts_handler_v0` and `stream_posts_handler_v1`: the page in
/// the version's shape, filtered by the kinds in the version's names.
pub async fn stream_posts<V: ApiVersion>(
    Query(mut query): Query<PostStreamQuery<V>>,
) -> AppResult<Json<V::PostStream>> {
    debug!("GET {STREAM_POSTS_ROUTE}");

    query.validate_source_compat()?; // before initialize_defaults
    query.initialize_defaults();
    let pagination = query.pagination.to_pagination(query.start, query.end);
    let (source, sorting, order) = query.extract_stream_params()?;
    let include_attachment_metadata = query.include_attachment_metadata.into();
    let tags = query.tags.as_ref().map(Tags::to_string_vec);

    let stream = match PostStream::get_posts(
        source,
        pagination,
        order,
        sorting,
        query.viewer_id.as_deref(),
        tags,
        query.kind_filter(),
    )
    .await?
    {
        Some(stream) => {
            PostStreamDetailed::from_post_views(stream.0, include_attachment_metadata).await?
        }
        None => PostStreamDetailed::default(),
    };
    Ok(Json(V::PostStream::from(stream)))
}

versioned_handlers! {
    stream_posts_handler_v0 / stream_posts_handler_v1 => stream_posts(query: Query<shapes::PostStreamQuery>) -> shapes::PostStream;
    #[utoipa::path(
        get,
        path = STREAM_POSTS_ROUTE,
        operation_id = "stream_posts_handler",
        tag = "Stream",
        params(
            ("source" = Option<shapes::StreamSource>, Query, description = concatcp!("Source of posts for streams with viewer (", shapes::SOURCE_NAMES, "). For `source=post_collections`: provide `author_id` + `post_id` of a post; returns the Collection posts that curate it, newest first by default. `kind`, `exclude_kinds` and `locked` are rejected with 400. Links are materialized when the Collection is written: an item indexed after the Collection that lists it is not linked until the Collection is edited. For `source=collection`: provide `author_id` + `post_id` of the Collection post; items are returned in curator order. `tags`, `kind`, `exclude_kinds`, `locked`, `sorting`, `order`, `start`, `end` are all rejected with 400 (incompatible with the curator-ordered result set). Items whose underlying post is missing (deleted, not indexed) or whose URI is malformed/non-post are dropped during hydration; pages may be shorter than `limit`. Pagination via `skip`/`limit` is not stable across deletions, if an item is removed between page fetches, the same `skip` returns a different window. The FE can identify dropped items by diffing the response against the Collection envelope's `items[]`. When the instance filters by trust, posts by authors outside its trust ranking are left out of `all` (any `sorting`, `tags` or `kind`).")),
            ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID. Sets the viewer's relationship flags on the posts. On `source=all` with `sorting=timeline` and no `tags`, also brings back the viewer's own posts when the trust filter hides them"),
            ("observer_id" = Option<PubkyId>, Query, description = "Observer Pubky ID. The central point for streams with Reach"),
            ("author_id" = Option<PubkyId>, Query, description = "Filter posts by an specific author User ID"),
            ("post_id" = Option<PostId>, Query, description = "This parameter is needed when we want to retrieve the replies stream for a post"),
            ("sorting" = Option<StreamSorting>, Query, description = "Sort method (`timeline` or `total_engagement`). Ties are broken by post id; pagination across equal scores is best-effort."),
            ("order" = Option<SortOrder>, Query, description = "Ordering of response list. Either 'ascending' or 'descending'. Defaults to descending."),
            ("tags" = Option<Tags>, Query, description = "Filter by a list of comma-separated tags (max 5). E.g.,`&tags=dev,free,opensource`. Only posts matching at least one of the tags will be returned."),
            ("depth" = Option<u8>, Query, description = "WoT traversal depth. For `source=wot`: 1-3, default 2. For `source=wot_domain`: 0-3, default 2, where `depth=0` is the observer-only (\"Me\") trust set (posts by authors the observer tagged directly, no follow traversal). `depth=0` is invalid for `source=wot`. Ignored for other sources."),
            ("domain_tags" = Option<Tags>, Query, description = "Required for `source=wot_domain`. Comma-separated tag labels (max 5); returns posts by authors tagged with any of these by the observer's WoT, or by the observer alone when `depth=0`. E.g. `&domain_tags=bitcoiner,btc-dev`. Ignored for other sources."),
            ("kind" = Option<shapes::PostKind>, Query, description = concatcp!("Filter by post kind: ", shapes::KIND_NAMES, shapes::KIND_RULE, ". Mutually exclusive with `exclude_kinds`; rejected for `source=post_collections`, `source=post_replies` and `source=author_replies`.")),
            ("exclude_kinds" = Option<shapes::PostKinds>, Query, description = concatcp!("Comma-separated post kinds to exclude server-side (", shapes::EXCLUDE_KINDS_RANGE, " items, duplicates ignored), e.g. `&exclude_kinds=collection,link`. Valid values: ", shapes::KIND_NAMES, "; anything else is rejected with 400. Mutually exclusive with `kind`; rejected for `source=collection`, `source=post_collections`, `source=post_replies` and `source=author_replies`. ", shapes::MISSING_KIND_RULE)),
            ("locked" = Option<bool>, Query, description = concatcp!("When `true`, return only posts with a lock attached, whoever the viewer is and whether or not they unlocked them. `false` (default) applies no filter. ", shapes::LOCKED_REPLIES_RULE, " Cannot be combined with `kind` or `exclude_kinds`; rejected for `source=collection`, `source=post_collections`, `source=post_replies` and `source=author_replies`.")),
            ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Skip N posts (max 10000)"),
            ("limit" = Option<BoundedLimit<10, 50>>, Query, description = "Retrieve N posts (1–50, default 10)"),
            ("start" = Option<f64>, Query, description = "The start of the stream timeframe or score. Posts with a timestamp/score greater than this value will be excluded from the results"),
            ("end" = Option<f64>, Query, description = "The end of the stream timeframe or score. Posts with a timestamp/score less than this value will be excluded from the results"),
            shapes::IncludeAttachmentMetadata,
        ),
        description = concatcp!(r#"Stream Posts: Retrieve a stream of posts.


The `source` parameter determines the type of stream. Depending on the `source`, certain parameters are required:
- *following*, *followers*, *friends*: Require **observer_id** and exclude posts authored by the observer.
"#, shapes::BOOKMARKS_SOURCE_LINE, r#"- *post_replies*: Requires **author_id** and **post_id** to filter replies to a specific post.
- *author*:  Requires  **author_id** to filter posts by a specific author.
- *author_replies*:  Requires  **author_id** to filter replies by a specific author.
- *collection*: Requires **author_id** and **post_id** of the Collection post; items are returned in curator order.
- *post_collections*: Requires **author_id** and **post_id** of a post; returns the Collection posts that contain it.

- *wot*: Requires **observer_id**. Posts from users in the observer's Web of Trust (transitive follows, `depth` 1-3, default 2). Excludes the observer's own posts, including when a follow cycle reaches the observer again.
- *wot_domain*: Requires **observer_id** and **domain_tags**. Posts by authors whom the observer's Web of Trust has tagged with any of `domain_tags`, all of those authors' posts, not only topic-tagged ones; combine with `tags=` for topic-scoped posts. With `depth=0` the trust set is the observer alone ("Me"): posts by authors the observer tagged directly. Includes the observer's own posts when they themselves are tagged with a matching label.

Ensure that you provide the necessary parameters based on the selected `source`. If a required parameter is missing, a 400 Bad Request error will be returned."#)
        )]
        responses(
            (status = 200, description = "Posts stream"),
            (status = 400, description = "Invalid parameters"),
            (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
            (status = 500, description = "Internal server error")
        )
}

/// The one body behind `stream_post_keys_handler_v0` and `stream_post_keys_handler_v1`: the
/// keys of the page, filtered by the kinds in the version's names.
pub async fn stream_post_keys<V: ApiVersion>(
    Query(mut query): Query<PostStreamQuery<V>>,
) -> AppResult<Json<PostKeyStream>> {
    debug!("GET {STREAM_POST_KEYS_ROUTE}");

    query.validate_source_compat()?; // before initialize_defaults
    query.initialize_defaults();
    let pagination = query.pagination.to_pagination(query.start, query.end);
    let (source, sorting, order) = query.extract_stream_params()?;
    let tags = query.tags.as_ref().map(Tags::to_string_vec);

    match PostStream::get_post_keys(
        source,
        pagination,
        order,
        sorting,
        query.viewer_id.as_deref(),
        tags,
        query.kind_filter(),
    )
    .await?
    {
        Some(stream) => Ok(Json(stream)),
        None => Ok(Json(PostKeyStream::default())),
    }
}

versioned_handlers! {
    stream_post_keys_handler_v0 / stream_post_keys_handler_v1 => stream_post_keys(query: Query<shapes::PostStreamQuery>) -> PostKeyStream;
    #[utoipa::path(
    get,
    path = STREAM_POST_KEYS_ROUTE,
    operation_id = "stream_post_keys_handler",
    tag = "Stream",
    params(
        ("source" = Option<shapes::StreamSource>, Query, description = concatcp!("Source of posts for streams with viewer (", shapes::SOURCE_NAMES, "). For `source=post_collections`: provide `author_id` + `post_id` of a post; returns the Collection posts that curate it, newest first by default. `kind`, `exclude_kinds` and `locked` are rejected with 400. Links are materialized when the Collection is written: an item indexed after the Collection that lists it is not linked until the Collection is edited. For `source=collection`: provide `author_id` + `post_id` of the Collection post; keys are returned in curator order. `tags`, `kind`, `exclude_kinds`, `locked`, `sorting`, `order`, `start`, `end` are all rejected with 400 (incompatible with the curator-ordered result set). Like every other source, the returned keys are a best-effort snapshot, they may reference posts that have since been deleted or are not yet indexed; callers should hydrate via `GET /v0/stream/posts?source=collection&author_id=...&post_id=...` (or `POST /v0/stream/posts/by_ids`) which drops unresolved refs. Pagination via `skip`/`limit` is not stable across deletions. When the instance filters by trust, posts by authors outside its trust ranking are left out of `all` (any `sorting`, `tags` or `kind`).")),
        ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID. On `source=all` with `sorting=timeline` and no `tags`, brings back the viewer's own posts when the trust filter hides them"),
        ("observer_id" = Option<PubkyId>, Query, description = "Observer Pubky ID. The central point for streams with Reach"),
        ("author_id" = Option<PubkyId>, Query, description = "Filter posts by an specific author User ID"),
        ("post_id" = Option<PostId>, Query, description = "This parameter is needed when we want to retrieve the replies stream for a post"),
        ("sorting" = Option<StreamSorting>, Query, description = "Sort method (`timeline` or `total_engagement`). Ties are broken by post id; pagination across equal scores is best-effort."),
        ("order" = Option<SortOrder>, Query, description = "Ordering of response list. Either 'ascending' or 'descending'. Defaults to descending."),
        ("tags" = Option<Tags>, Query, description = "Filter by a list of comma-separated tags (max 5). E.g.,`&tags=dev,free,opensource`. Only posts matching at least one of the tags will be returned."),
        ("depth" = Option<u8>, Query, description = "WoT traversal depth. For `source=wot`: 1-3, default 2. For `source=wot_domain`: 0-3, default 2, where `depth=0` is the observer-only (\"Me\") trust set (posts by authors the observer tagged directly, no follow traversal). `depth=0` is invalid for `source=wot`. Ignored for other sources."),
        ("domain_tags" = Option<Tags>, Query, description = "Required for `source=wot_domain`. Comma-separated tag labels (max 5); returns posts by authors tagged with any of these by the observer's WoT, or by the observer alone when `depth=0`. E.g. `&domain_tags=bitcoiner,btc-dev`. Ignored for other sources."),
        ("kind" = Option<shapes::PostKind>, Query, description = concatcp!("Filter by post kind: ", shapes::KIND_NAMES, shapes::KIND_RULE, ". Mutually exclusive with `exclude_kinds`; rejected for `source=post_collections`, `source=post_replies` and `source=author_replies`.")),
        ("exclude_kinds" = Option<shapes::PostKinds>, Query, description = concatcp!("Comma-separated post kinds to exclude server-side (", shapes::EXCLUDE_KINDS_RANGE, " items, duplicates ignored), e.g. `&exclude_kinds=collection,link`. Valid values: ", shapes::KIND_NAMES, "; anything else is rejected with 400. Mutually exclusive with `kind`; rejected for `source=collection`, `source=post_collections`, `source=post_replies` and `source=author_replies`. ", shapes::MISSING_KIND_RULE)),
        ("locked" = Option<bool>, Query, description = concatcp!("When `true`, return only posts with a lock attached, whoever the viewer is and whether or not they unlocked them. `false` (default) applies no filter. ", shapes::LOCKED_REPLIES_RULE, " Cannot be combined with `kind` or `exclude_kinds`; rejected for `source=collection`, `source=post_collections`, `source=post_replies` and `source=author_replies`.")),
        ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Skip N posts (max 10000)"),
        ("limit" = Option<BoundedLimit<10, 50>>, Query, description = "Retrieve N posts (1–50, default 10)"),
        ("start" = Option<f64>, Query, description = "The start of the stream timeframe or score. Posts with a timestamp/score greater than this value will be excluded from the results"),
        ("end" = Option<f64>, Query, description = "The end of the stream timeframe or score. Posts with a timestamp/score less than this value will be excluded from the results"),
    ),
    description = concatcp!(r#"Stream Post Keys: Retrieve a stream of post keys

The `source` parameter determines the type of stream. Depending on the `source`, certain parameters are required:
- *following*, *followers*, *friends*: Require **observer_id** and exclude posts authored by the observer.
"#, shapes::BOOKMARKS_SOURCE_LINE, r#"- *post_replies*: Requires **author_id** and **post_id** to filter replies to a specific post.
- *author*:  Requires  **author_id** to filter posts by a specific author.
- *author_replies*:  Requires  **author_id** to filter replies by a specific author.
- *collection*: Requires **author_id** and **post_id** of the Collection post; keys are returned in curator order.
- *post_collections*: Requires **author_id** and **post_id** of a post; returns the keys of the Collection posts that contain it.

- *wot*: Requires **observer_id**. Posts from users in the observer's Web of Trust (transitive follows, `depth` 1-3, default 2). Excludes the observer's own posts, including when a follow cycle reaches the observer again.
- *wot_domain*: Requires **observer_id** and **domain_tags**. Posts by authors whom the observer's Web of Trust has tagged with any of `domain_tags`, all of those authors' posts, not only topic-tagged ones; combine with `tags=` for topic-scoped posts. With `depth=0` the trust set is the observer alone ("Me"): posts by authors the observer tagged directly. Includes the observer's own posts when they themselves are tagged with a matching label.

Ensure that you provide the necessary parameters based on the selected `source`. If a required parameter is missing, a 400 Bad Request error will be returned."#)
    )]
    responses(
        (status = 200, description = "Post key stream"),
        (status = 400, description = "Invalid parameters"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
}

// The body of `POST /stream/posts/by_ids` as `/v0` reads it and as the one handler body reads
// it: `/v1` reads `v1::PostStreamByIdsRequest`, the same body without
// `include_attachment_metadata`, and converts into this. Not a doc comment: that would become
// the frozen `/v0` schema's description.
#[derive(ToSchema, Deserialize)]
pub struct PostStreamByIdsRequest {
    pub post_ids: GlobalPostIds,
    pub viewer_id: Option<PubkyId>,
    #[serde(default)]
    pub include_attachment_metadata: bool,
}

/// The one body behind `stream_posts_by_ids_handler_v0` and `stream_posts_by_ids_handler_v1`:
/// the listed posts in the version's shape, from the body in the version's shape.
pub async fn stream_posts_by_ids<V: ApiVersion>(
    RequestJson(request): RequestJson<V::PostStreamByIdsRequest>,
) -> AppResult<Json<V::PostStream>> {
    let request: PostStreamByIdsRequest = request.into();
    debug!(
        "POST {} post_ids size {:?}",
        STREAM_POSTS_BY_IDS_ROUTE,
        request.post_ids.0.len()
    );

    let post_ids = request.post_ids.into_string_vec();

    let stream =
        match PostStream::from_listed_post_ids(request.viewer_id.as_deref(), &post_ids).await? {
            Some(stream) => {
                PostStreamDetailed::from_post_views(stream.0, request.include_attachment_metadata)
                    .await?
            }
            None => PostStreamDetailed::default(),
        };
    Ok(Json(V::PostStream::from(stream)))
}

versioned_handlers! {
    stream_posts_by_ids_handler_v0 / stream_posts_by_ids_handler_v1 => stream_posts_by_ids(request: RequestJson<shapes::PostStreamByIdsRequest>) -> shapes::PostStream;
    #[utoipa::path(
        post,
        path = STREAM_POSTS_BY_IDS_ROUTE,
        operation_id = "stream_posts_by_ids_handler",
        tag = "Stream",
        description = "Stream post by ID. This is a POST request because we're passing a potentially large list of post IDs in the request body",
        request_body = shapes::PostStreamByIdsRequest
    )]
    responses(
        (status = 200, description = "Post stream"),
        (status = 400, description = "Invalid request body"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
}

/// The schemas the post stream routes share; the routes themselves, their source and kind
/// filters and the `by_ids` body come with the version's document.
#[derive(OpenApi)]
#[openapi(components(schemas(
    PostKeyStream,
    StreamSorting,
    SortOrder,
    PubkyId,
    PostId,
    Tags,
    GlobalPostId
)))]
pub struct StreamPostsApiDocs;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::v0::version::{V0, V1};
    use nexus_common::models::post::PostKind;

    fn parse<V: ApiVersion>(
        query: &str,
    ) -> std::result::Result<PostStreamQuery<V>, serde_urlencoded::de::Error> {
        serde_urlencoded::from_str(query)
    }

    /// The filter a query maps to, in the stored names whichever version parsed it.
    fn kind_filter<V: ApiVersion>(query: &str) -> Option<KindFilter> {
        parse::<V>(query).expect(query).kind_filter()
    }

    /// `/v0` reads the stored names, and any other name in `kind=` as `unknown`.
    #[test]
    fn v0_kind_is_lenient_and_exclude_kinds_strict() {
        assert!(matches!(
            kind_filter::<V0>("kind=short"),
            Some(KindFilter::Kind(PostKind::Short))
        ));
        assert!(matches!(
            kind_filter::<V0>("kind=note"),
            Some(KindFilter::Kind(PostKind::Unknown))
        ));
        assert!(matches!(
            kind_filter::<V0>("exclude_kinds=short,long"),
            Some(KindFilter::Exclude(kinds)) if kinds == [PostKind::Short, PostKind::Long]
        ));
        assert!(parse::<V0>("exclude_kinds=note").is_err());
        assert!(parse::<V0>("exclude_kinds=unknown").is_err());
    }

    /// `/v1` reads the `social/v1` names, `unknown` included, into the stored kinds.
    #[test]
    fn v1_kind_filters_take_the_v1_names() {
        assert!(matches!(
            kind_filter::<V1>("kind=note"),
            Some(KindFilter::Kind(PostKind::Short))
        ));
        assert!(matches!(
            kind_filter::<V1>("kind=article"),
            Some(KindFilter::Kind(PostKind::Long))
        ));
        assert!(matches!(
            kind_filter::<V1>("kind=unknown"),
            Some(KindFilter::Kind(PostKind::Unknown))
        ));
        assert!(matches!(
            kind_filter::<V1>("exclude_kinds=note,article"),
            Some(KindFilter::Exclude(kinds)) if kinds == [PostKind::Short, PostKind::Long]
        ));
        assert!(matches!(
            kind_filter::<V1>("exclude_kinds=unknown"),
            Some(KindFilter::Exclude(kinds)) if kinds == [PostKind::Unknown]
        ));
        assert!(kind_filter::<V1>("").is_none());
    }

    /// On `/v1` a stored name, an unknown token and a differently cased name are errors that
    /// name the parameter, on `kind=` and `exclude_kinds=` alike.
    #[test]
    fn v1_rejects_other_kind_names_naming_the_parameter() {
        for (query, parameter) in [
            ("kind=short", "`kind`"),
            ("kind=foo", "`kind`"),
            ("kind=Note", "`kind`"),
            ("exclude_kinds=short", "`exclude_kinds`"),
            ("exclude_kinds=note,long", "`exclude_kinds`"),
        ] {
            let error = parse::<V1>(query).expect_err(query).to_string();
            assert!(error.contains(parameter), "{query}: {error}");
            assert!(error.contains(V1::KIND_NAMES), "{query}: {error}");
        }
    }

    /// The rejections that hold on both versions: `kind` with `exclude_kinds`, and a kind
    /// filter on the sources that cannot take one.
    #[test]
    fn kind_filter_rejections_hold_on_both_versions() {
        fn rejected<V: ApiVersion>(query: &str) -> bool {
            parse::<V>(query)
                .expect(query)
                .validate_source_compat()
                .is_err()
        }
        let author = "author_id=4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
        let anchored = format!("{author}&post_id=0000000000001");
        // The same queries in each version's names.
        for (short, long, rejected) in [
            ("short", "long", rejected::<V0> as fn(&str) -> bool),
            ("note", "article", rejected::<V1>),
        ] {
            for query in [
                format!("kind={short}&exclude_kinds={long}"),
                format!("source=post_collections&{anchored}&kind={short}"),
                format!("source=post_replies&{anchored}&exclude_kinds={short}"),
                format!("source=author_replies&{author}&kind={long}"),
                format!("source=collection&{anchored}&kind={short}"),
            ] {
                assert!(rejected(&query), "{query}");
            }
        }
        assert!(!rejected::<V0>("kind=short"));
        assert!(!rejected::<V1>("kind=note"));
    }

    #[test]
    fn resolve_domain_trust_maps_depth() {
        // Absent depth defaults to the network reach (backward compatible).
        assert_eq!(
            resolve_domain_trust(None).unwrap(),
            DomainTrust::Network(WotDepth::default())
        );
        // depth=0 selects the observer-only "Me" trust set.
        assert_eq!(resolve_domain_trust(Some(0)).unwrap(), DomainTrust::Me);
        // 1..=3 are network reaches.
        for d in 1..=3 {
            assert_eq!(
                resolve_domain_trust(Some(d)).unwrap(),
                DomainTrust::Network(WotDepth::new(d).unwrap())
            );
        }
        // Out of range is rejected (unlike depth=0, which is valid here).
        assert!(resolve_domain_trust(Some(4)).is_err());
    }
}
