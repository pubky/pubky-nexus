use crate::routes::v0::endpoints::{
    STREAM_POSTS_BY_IDS_ROUTE, STREAM_POSTS_ROUTE, STREAM_POST_KEYS_ROUTE,
};
use crate::{Error, Result as AppResult};
use axum::{extract::Query, Json};
use nexus_common::db::kv::SortOrder;
use nexus_common::types::StreamSorting;
use nexus_common::{
    models::post::{PostKeyStream, PostStream, StreamSource},
    types::Pagination,
};
use pubky_app_specs::PubkyAppPostKind;
use serde::{de, Deserialize, Deserializer};
use tracing::info;
use utoipa::{OpenApi, ToSchema};

const MAX_TAGS: usize = 5;

// Parameter description constants for OpenAPI documentation
const PARAM_SOURCE_DESC: &str = "Source of posts for streams with viewer (following, followers, friends, bookmarks, replies, all)";
const PARAM_VIEWER_ID_DESC: &str = "Viewer Pubky ID";
const PARAM_OBSERVER_ID_DESC: &str = "Observer Pubky ID. The central point for streams with Reach";
const PARAM_AUTHOR_ID_DESC: &str = "Filter posts by an specific author User ID";
const PARAM_POST_ID_DESC: &str =
    "This parameter is needed when we want to retrieve the replies stream for a post";
const PARAM_SORTING_DESC: &str = "StreamSorting method";
const PARAM_ORDER_DESC: &str =
    "Ordering of response list. Either 'ascending' or 'descending'. Defaults to descending.";
const PARAM_TAGS_DESC: &str = "Filter by a list of comma-separated tags (max 5). E.g.,`&tags=dev,free,opensource`. Only posts matching at least one of the tags will be returned.";
const PARAM_KIND_DESC: &str =
    "Specifies the type of posts to retrieve: short, long, image, video, link and file";
const PARAM_SKIP_DESC: &str = "Skip N posts";
const PARAM_LIMIT_DESC: &str = "Retrieve N posts";
const PARAM_START_DESC: &str = "The start of the stream timeframe or score. Posts with a timestamp/score greater than this value will be excluded from the results";
const PARAM_END_DESC: &str = "The end of the stream timeframe or score. Posts with a timestamp/score less than this value will be excluded from the results";

macro_rules! stream_desc {
    ($intro:literal) => {
        concat!(
            $intro,
            "\n",
            "\n",
            "The `source` parameter determines the type of stream. Depending on the `source`, certain parameters are required:\n",
            "- *following*, *followers*, *friends*, *bookmarks*: Requires **observer_id**.\n",
            "- *post_replies*: Requires **author_id** and **post_id** to filter replies to a specific post.\n",
            "- *author*:  Requires  **author_id** to filter posts by a specific author.\n",
            "- *author_replies*:  Requires  **author_id** to filter replies by a specific author.\n",
            "\n",
            "Ensure that you provide the necessary parameters based on the selected `source`. If the required parameter is not provided, the provided `source` will be ignored and the stream type will default to *all*"
        )
    };
}

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

    pub fn extract_stream_params(&self) -> (StreamSource, StreamSorting, SortOrder) {
        (
            self.source.as_ref().cloned().unwrap_or_default(), // StreamSource::All is default
            self.sorting.as_ref().cloned().unwrap_or_default(), // StreamSorting::Timeline is default
            self.order.as_ref().cloned().unwrap_or_default(),   // SortOrder::Descending is default
        )
    }

    pub fn validate_tags(&self) -> AppResult<()> {
        if let Some(ref tags) = self.tags {
            if tags.len() > MAX_TAGS {
                return Err(Error::InvalidInput {
                    message: format!("Too many tags provided; maximum allowed is {MAX_TAGS}"),
                });
            }
        }
        Ok(())
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
        ("source" = Option<StreamSource>, Query, description = PARAM_SOURCE_DESC),
        ("viewer_id" = Option<String>, Query, description = PARAM_VIEWER_ID_DESC),
        ("observer_id" = Option<String>, Query, description = PARAM_OBSERVER_ID_DESC),
        ("author_id" = Option<String>, Query, description = PARAM_AUTHOR_ID_DESC),
        ("post_id" = Option<String>, Query, description = PARAM_POST_ID_DESC),
        ("sorting" = Option<StreamSorting>, Query, description = PARAM_SORTING_DESC),
        ("order" = Option<SortOrder>, Query, description = PARAM_ORDER_DESC),
        ("tags" = Option<Vec<String>>, Query, description = PARAM_TAGS_DESC),
        ("kind" = Option<PubkyAppPostKind>, Query, description = PARAM_KIND_DESC),
        ("skip" = Option<usize>, Query, description = PARAM_SKIP_DESC),
        ("limit" = Option<usize>, Query, description = PARAM_LIMIT_DESC),
        ("start" = Option<usize>, Query, description = PARAM_START_DESC),
        ("end" = Option<usize>, Query, description = PARAM_END_DESC),
    ),
    responses(
        (status = 200, description = "Posts stream", body = PostStream),
        (status = 404, description = "Posts not found"),
        (status = 500, description = "Internal server error")
    ),
    description = stream_desc!("Stream Posts: Retrieve a stream of posts.\n")
)]
pub async fn stream_posts_handler(
    Query(mut query): Query<PostStreamQuery>,
) -> AppResult<Json<PostStream>> {
    info!("GET {STREAM_POSTS_ROUTE}");

    query.initialize_defaults();
    query.validate_tags()?;
    let (source, sorting, order) = query.extract_stream_params();

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

#[utoipa::path(
    get,
    path = STREAM_POST_KEYS_ROUTE,
    tag = "Stream",
    params(
        ("source" = Option<StreamSource>, Query, description = PARAM_SOURCE_DESC),
        ("observer_id" = Option<String>, Query, description = PARAM_OBSERVER_ID_DESC),
        ("author_id" = Option<String>, Query, description = PARAM_AUTHOR_ID_DESC),
        ("post_id" = Option<String>, Query, description = PARAM_POST_ID_DESC),
        ("sorting" = Option<StreamSorting>, Query, description = PARAM_SORTING_DESC),
        ("order" = Option<SortOrder>, Query, description = PARAM_ORDER_DESC),
        ("tags" = Option<Vec<String>>, Query, description = PARAM_TAGS_DESC),
        ("kind" = Option<PubkyAppPostKind>, Query, description = PARAM_KIND_DESC),
        ("skip" = Option<usize>, Query, description = PARAM_SKIP_DESC),
        ("limit" = Option<usize>, Query, description = PARAM_LIMIT_DESC),
        ("start" = Option<usize>, Query, description = PARAM_START_DESC),
        ("end" = Option<usize>, Query, description = PARAM_END_DESC),
    ),
    responses(
        (status = 200, description = "Post key stream", body = PostKeyStream),
        (status = 404, description = "Posts not found"),
        (status = 500, description = "Internal server error")
    ),
    description = stream_desc!("Stream Post Keys: Retrieve a stream of post identifiers")
)]
pub async fn stream_post_keys_handler(
    Query(mut query): Query<PostStreamQuery>,
) -> AppResult<Json<PostKeyStream>> {
    info!("GET {STREAM_POST_KEYS_ROUTE}");

    query.initialize_defaults();
    query.validate_tags()?;
    let (source, sorting, order) = query.extract_stream_params();

    match PostStream::get_post_keys(
        source,
        query.pagination,
        order,
        sorting,
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
    paths(
        stream_posts_handler,
        stream_post_keys_handler,
        stream_posts_by_ids_handler
    ),
    components(schemas(PostKeyStream, PostStream, StreamSorting, StreamSource))
)]
pub struct StreamPostsApiDocs;
