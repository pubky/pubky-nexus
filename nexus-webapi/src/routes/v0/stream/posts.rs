use crate::models::{GlobalPostId, GlobalPostIds, PostId, PostStreamDetailed, PubkyId, Tags};
use crate::routes::v0::endpoints::{
    STREAM_POSTS_BY_IDS_ROUTE, STREAM_POSTS_ROUTE, STREAM_POST_KEYS_ROUTE,
};
use crate::routes::ValidJson;
use crate::Result as AppResult;
use axum::{extract::Query, Json};
use nexus_common::db::kv::SortOrder;
use nexus_common::types::StreamSorting;
use nexus_common::{
    models::post::{PostKeyStream, PostStream, StreamSource},
    types::Pagination,
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
    #[default]
    All,
}

/// Convert a validated query into the internal StreamSource used by nexus-common.
fn build_stream_source(
    kind: &StreamSourceKind,
    author_id: Option<&PubkyId>,
    observer_id: Option<&PubkyId>,
    post_id: Option<&PostId>,
) -> StreamSource {
    match kind {
        StreamSourceKind::PostReplies => StreamSource::PostReplies {
            post_id: post_id
                .expect("post_id required for post_replies")
                .to_string(),
            author_id: author_id
                .expect("author_id required for post_replies")
                .to_string(),
        },
        StreamSourceKind::Following => StreamSource::Following {
            observer_id: observer_id
                .expect("observer_id required for following")
                .to_string(),
        },
        StreamSourceKind::Followers => StreamSource::Followers {
            observer_id: observer_id
                .expect("observer_id required for followers")
                .to_string(),
        },
        StreamSourceKind::Friends => StreamSource::Friends {
            observer_id: observer_id
                .expect("observer_id required for friends")
                .to_string(),
        },
        StreamSourceKind::Bookmarks => StreamSource::Bookmarks {
            observer_id: observer_id
                .expect("observer_id required for bookmarks")
                .to_string(),
        },
        StreamSourceKind::Author => StreamSource::Author {
            author_id: author_id
                .expect("author_id required for author")
                .to_string(),
        },
        StreamSourceKind::AuthorReplies => StreamSource::AuthorReplies {
            author_id: author_id
                .expect("author_id required for author_replies")
                .to_string(),
        },
        StreamSourceKind::All => StreamSource::All,
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

    pub fn build_source(&self) -> StreamSource {
        build_stream_source(
            &self.source,
            self.author_id.as_ref(),
            self.observer_id.as_ref(),
            self.post_id.as_ref(),
        )
    }

    pub fn extract_stream_params(&self) -> (StreamSource, StreamSorting, SortOrder) {
        (
            self.build_source(),
            self.sorting.as_ref().cloned().unwrap_or_default(),
            self.order.as_ref().cloned().unwrap_or_default(),
        )
    }

    pub fn viewer_id_str(&self) -> Option<String> {
        self.viewer_id.as_ref().map(|id| id.to_string())
    }

    pub fn tags_as_strings(&self) -> Option<Vec<String>> {
        self.tags
            .as_ref()
            .map(|tags| tags.0.iter().map(|t| t.clone()).collect())
    }
}

#[utoipa::path(
    get,
    path = STREAM_POSTS_ROUTE,
    tag = "Stream",
    params(
        ("source" = StreamSourceKind, Query, description = "Source of posts for streams with viewer (following, followers, friends, bookmarks, post_replies, author, author_replies, all)"),
        ("viewer_id" = Option<PubkyId>, Query, description = "Viewer Pubky ID"),
        ("observer_id" = Option<PubkyId>, Query, description = "Observer Pubky ID. The central point for streams with Reach"),
        ("author_id" = Option<PubkyId>, Query, description = "Filter posts by an specific author User ID"),
        ("post_id" = Option<PostId>, Query, description = "This parameter is needed when we want to retrieve the replies stream for a post"),
        ("sorting" = Option<StreamSorting>, Query, description = "StreamSorting method"),
        ("order" = Option<SortOrder>, Query, description = "Ordering of response list. Either 'ascending' or 'descending'. Defaults to descending."),
        ("tags" = Option<Tags>, Query, description = "Filter by a list of comma-separated tags (max 5). E.g.,`&tags=dev,free,opensource`. Only posts matching at least one of the tags will be returned."),
        ("kind" = Option<PubkyAppPostKind>, Query, description = "Specifies the type of posts to retrieve: short, long, image, video, link and file"),
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

Ensure that you provide the necessary parameters based on the selected `source`. If the required parameter is not provided, the provided `source` will be ignored and the stream type will default to *all*"#
)]
pub async fn stream_posts_handler(
    Query(mut query): Query<PostStreamQuery>,
) -> AppResult<Json<PostStreamDetailed>> {
    debug!("GET {STREAM_POSTS_ROUTE}");

    query.initialize_defaults();
    let (source, sorting, order) = query.extract_stream_params();
    let include_attachment_metadata = query.include_attachment_metadata;
    let viewer_id = query.viewer_id_str();
    let tags = query.tags_as_strings();

    match PostStream::get_posts(
        source,
        query.pagination,
        order,
        sorting,
        viewer_id,
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
        ("source" = StreamSourceKind, Query, description = "Source of posts for streams with viewer (following, followers, friends, bookmarks, post_replies, author, author_replies, all)"),
        ("observer_id" = Option<PubkyId>, Query, description = "Observer Pubky ID. The central point for streams with Reach"),
        ("author_id" = Option<PubkyId>, Query, description = "Filter posts by an specific author User ID"),
        ("post_id" = Option<PostId>, Query, description = "This parameter is needed when we want to retrieve the replies stream for a post"),
        ("sorting" = Option<StreamSorting>, Query, description = "StreamSorting method"),
        ("order" = Option<SortOrder>, Query, description = "Ordering of response list. Either 'ascending' or 'descending'. Defaults to descending."),
        ("tags" = Option<Tags>, Query, description = "Filter by a list of comma-separated tags (max 5). E.g.,`&tags=dev,free,opensource`. Only posts matching at least one of the tags will be returned."),
        ("kind" = Option<PubkyAppPostKind>, Query, description = "Specifies the type of posts to retrieve: short, long, image, video, link and file"),
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

Ensure that you provide the necessary parameters based on the selected `source`. If the required parameter is not provided, the provided `source` will be ignored and the stream type will default to *all*"#
)]
pub async fn stream_post_keys_handler(
    Query(mut query): Query<PostStreamQuery>,
) -> AppResult<Json<PostKeyStream>> {
    debug!("GET {STREAM_POST_KEYS_ROUTE}");

    query.initialize_defaults();
    let (source, sorting, order) = query.extract_stream_params();
    let tags = query.tags_as_strings();

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
    ValidJson(request): ValidJson<PostStreamByIdsRequest>,
) -> AppResult<Json<PostStreamDetailed>> {
    debug!(
        "POST {} post_ids size {:?}",
        STREAM_POSTS_BY_IDS_ROUTE,
        request.post_ids.0.len()
    );

    let viewer_id = request.viewer_id.as_ref().map(|id| id.to_string());
    let post_ids = request.post_ids.into_string_vec();

    match PostStream::from_listed_post_ids(viewer_id, &post_ids).await? {
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
