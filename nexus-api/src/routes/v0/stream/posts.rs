use crate::routes::v0::endpoints::{STREAM_POSTS_BY_IDS_ROUTE, STREAM_POSTS_ROUTE};
use crate::{Error, Result as AppResult};
use axum::{extract::Query, Json};
use nexus_common::db::kv::SortOrder;
use nexus_common::types::StreamSorting;
use nexus_common::{
    models::post::{PostStream, StreamSource},
    types::Pagination,
};
use pubky_app_specs::PubkyAppPostKind;
use serde::{de, Deserialize, Deserializer};
use tracing::info;
use utoipa::{OpenApi, ToSchema};

const MAX_TAGS: usize = 5;

#[derive(Deserialize, Debug, ToSchema)]
pub struct PostStreamQuery {
    #[serde(flatten, default)]
    pub source: Option<StreamSource>,
    #[serde(flatten)]
    pub pagination: Pagination,
    pub order: Option<SortOrder>,
    pub sorting: Option<StreamSorting>,
    pub viewer_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_comma_separated")]
    pub tags: Option<Vec<String>>,
    pub kind: Option<PubkyAppPostKind>,
}

impl PostStreamQuery {
    pub fn initialize_defaults(&mut self) {
        self.pagination.skip.get_or_insert(0);
        self.pagination.limit = Some(self.pagination.limit.unwrap_or(10).min(30));
        self.sorting.get_or_insert(StreamSorting::Timeline);
    }
}

// Custom deserializer for comma-separated tags
fn deserialize_comma_separated<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    if let Some(s) = s {
        if s.is_empty() {
            return Err(de::Error::custom("Tags cannot be empty"));
        }
        // Split by comma and trim any excess whitespace
        let tags: Vec<String> = s.split(',').map(|tag| tag.trim().to_string()).collect();
        return Ok(Some(tags));
    }
    Ok(None)
}

#[utoipa::path(
    get,
    path = STREAM_POSTS_ROUTE,
    tag = "Stream",
    params(
        ("source" = Option<StreamSource>, Query, description = "Source of posts for streams with viewer (following, followers, friends, bookmarks, replies, all)"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
        ("observer_id" = Option<String>, Query, description = "Observer Pubky ID. The central point for streams with Reach"),
        ("author_id" = Option<String>, Query, description = "Filter posts by an specific author User ID"),
        ("post_id" = Option<String>, Query, description = "This parameter is needed when we want to retrieve the replies stream for a post"),
        ("sorting" = Option<StreamSorting>, Query, description = "StreamSorting method"),
        ("order" = Option<SortOrder>, Query, description = "Ordering of response list. Either 'ascending' or 'descending'. Defaults to descending."),
        ("tags" = Option<Vec<String>>, Query, description = "Filter by a list of comma-separated tags (max 5). E.g.,`&tags=dev,free,opensource`. Only posts matching at least one of the tags will be returned."),
        ("kind" = Option<PubkyAppPostKind>, Query, description = "Specifies the type of posts to retrieve: short, long, image, video, link and file"),
        ("skip" = Option<usize>, Query, description = "Skip N posts"),
        ("limit" = Option<usize>, Query, description = "Retrieve N posts"),
        ("start" = Option<usize>, Query, description = "The start of the stream timeframe or score. Posts with a timestamp/score greater than this value will be excluded from the results"),
        ("end" = Option<usize>, Query, description = "The end of the stream timeframe or score. Posts with a timestamp/score less than this value will be excluded from the results"),
    ),
    responses(
        (status = 200, description = "Posts stream", body = PostStream),
        (status = 404, description = "Posts not found"),
        (status = 500, description = "Internal server error")
    ),
    description = "Stream Posts
    
    Retrieve a stream of posts. The `source` parameter determines the type of stream. Depending on the `source`, certain parameters are required:

    - `following`, `followers`, `friends`, `bookmarks`: Requires `observer_id`.
    - `post_replies`: Requires `author_id` and `post_id` to filter replies to a specific post.
    - `author`:  Requires  `author_id` to filter posts by a specific author.
    - `author_replies`:  Requires  `author_id` to filter replies by a specific author.
    
    Ensure that you provide the necessary parameters based on the selected `source`. If the required parameter is not
    provided, the provided `source` will be ignored and the stream type will default to `all`"
)]
pub async fn stream_posts_handler(
    Query(mut query): Query<PostStreamQuery>,
) -> AppResult<Json<PostStream>> {
    info!("GET {STREAM_POSTS_ROUTE}");

    query.initialize_defaults();

    // Enforce maximum number of tags
    if let Some(ref tags) = query.tags {
        if tags.len() > MAX_TAGS {
            return Err(Error::InvalidInput {
                message: format!("Too many tags provided; maximum allowed is {MAX_TAGS}"),
            });
        }
    }

    let source = query.source.unwrap_or_default(); // StreamSource::All is default
    let sorting = query.sorting.unwrap_or_default(); // StreamSorting::Timeline is default
    let order = query.order.unwrap_or_default(); // SortOrder::Descending is default

    match PostStream::get_posts(
        source,
        query.pagination,
        order,
        sorting,
        query.viewer_id,
        query.tags,
        query.kind,
    )
    .await
    {
        Ok(Some(stream)) => Ok(Json(stream)),
        Ok(None) => Err(Error::EmptyStream {
            message: "No posts found for the given criteria".to_string(),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(ToSchema, Deserialize)]
pub struct PostStreamByIdsRequest {
    pub post_ids: Vec<String>,
    pub viewer_id: Option<String>,
}
#[utoipa::path(
    post,
    path = STREAM_POSTS_BY_IDS_ROUTE,
    tag = "Stream",
    description = "Stream post by ID. This is a POST request because we're passing a potentially large list of post IDs in the request body",
    request_body = PostStreamByIdsRequest,
    params(
        ("post_ids" = Vec<String>, Path, description = "Post ID array"),
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID")
    ),
    responses(
        (status = 200, description = "Post stream", body = PostStream),
        (status = 404, description = "Posts not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn stream_posts_by_ids_handler(
    Json(request): Json<PostStreamByIdsRequest>,
) -> AppResult<Json<PostStream>> {
    info!(
        "POST {} post_ids size {:?}",
        STREAM_POSTS_BY_IDS_ROUTE,
        request.post_ids.len()
    );

    const MAX_POSTS: usize = 100;

    if request.post_ids.len() > MAX_POSTS {
        return Err(Error::InvalidInput {
            message: format!("The maximum number of post IDs allowed is {MAX_POSTS}"),
        });
    }

    if request.post_ids.is_empty() {
        return Err(Error::InvalidInput {
            message: "The list of post IDs provided is empty".to_string(),
        });
    }

    match PostStream::from_listed_post_ids(request.viewer_id, &request.post_ids).await {
        Ok(Some(stream)) => Ok(Json(stream)),
        Ok(None) => Err(Error::EmptyStream {
            message: format!(
                "No users found for the requested stream with user ids: {:?}",
                request.post_ids
            ),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(stream_posts_handler, stream_posts_by_ids_handler),
    components(schemas(PostStream, StreamSorting, StreamSource))
)]
pub struct StreamPostsApiDocs;
