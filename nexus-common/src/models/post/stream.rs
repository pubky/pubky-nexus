use std::sync::Arc;

use super::{Bookmark, PostCounts, PostDetails, PostView};
use crate::db::kv::{RedisResult, ScoreAction, SortOrder};
use crate::db::{get_neo4j_graph, queries, GraphError, GraphResult, RedisOps};
use crate::models::error::ModelError;
use crate::models::error::ModelResult;
use crate::models::{
    follow::{Followers, Following, Friends, UserFollows},
    post::search::PostsByTagSearch,
};
use crate::types::{DomainTrust, Pagination, StreamSorting, WotDepth};
use futures::stream::{self, StreamExt};
use futures::TryStreamExt;
use pubky_app_specs::{ParsedUri, PubkyAppCollectionContent, PubkyAppPostKind, Resource};
use serde::{Deserialize, Serialize};
use tokio::task::spawn;
use tokio::time::{timeout, Duration};
use tracing::warn;
use utoipa::ToSchema;

pub const POST_TIMELINE_KEY_PARTS: [&str; 3] = ["Posts", "Global", "Timeline"];
pub const POST_TOTAL_ENGAGEMENT_KEY_PARTS: [&str; 3] = ["Posts", "Global", "TotalEngagement"];
pub const POST_PER_USER_KEY_PARTS: [&str; 2] = ["Posts", "AuthorParents"];
pub const POST_REPLIES_PER_USER_KEY_PARTS: [&str; 2] = ["Posts", "AuthorReplies"];
pub const POST_REPLIES_PER_POST_KEY_PARTS: [&str; 2] = ["Posts", "PostReplies"];
const BOOKMARKS_USER_KEY_PARTS: [&str; 2] = ["Bookmarks", "User"];

#[derive(ToSchema, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum StreamSource {
    PostReplies {
        post_id: String,
        author_id: String,
    },
    Following {
        observer_id: String,
    },
    Followers {
        observer_id: String,
    },
    Friends {
        observer_id: String,
    },
    Bookmarks {
        observer_id: String,
    },
    Author {
        author_id: String,
    },
    AuthorReplies {
        author_id: String,
    },
    Collection {
        author_id: String,
        post_id: String,
    },
    /// Posts authored by users in the observer's Web of Trust (transitive FOLLOWS, 1..=depth).
    Wot {
        observer_id: String,
        depth: WotDepth,
    },
    /// Posts by users whom the observer's Web of Trust has tagged with a `domain_tags` label.
    /// `trust = Me` restricts the taggers to the observer alone (depth-0 self set).
    WotDomain {
        observer_id: String,
        trust: DomainTrust,
        domain_tags: Vec<String>,
    },
    #[default]
    All,
}

impl StreamSource {
    pub fn get_observer(&self) -> Option<&str> {
        match self {
            StreamSource::Followers { observer_id }
            | StreamSource::Following { observer_id }
            | StreamSource::Friends { observer_id }
            | StreamSource::Bookmarks { observer_id }
            | StreamSource::Wot { observer_id, .. }
            | StreamSource::WotDomain { observer_id, .. } => Some(observer_id),
            _ => None,
        }
    }

    /// Domain-trust tag labels carried by `WotDomain`; `None` for other sources.
    pub fn get_domain_tags(&self) -> Option<&[String]> {
        match self {
            StreamSource::WotDomain { domain_tags, .. } => Some(domain_tags),
            _ => None,
        }
    }

    /// Author whose posts are streamed. Collection returns `None`: its
    /// `author_id` is the curator, not the items' authors.
    pub fn get_author(&self) -> Option<&str> {
        match self {
            StreamSource::PostReplies {
                author_id,
                post_id: _,
            } => Some(author_id),
            StreamSource::Author { author_id } => Some(author_id),
            StreamSource::AuthorReplies { author_id } => Some(author_id),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Default, Clone)]
#[serde(rename_all = "snake_case")]
pub struct PostKeyStream {
    pub post_keys: Vec<String>,
    pub last_post_score: Option<u64>,
}

impl PostKeyStream {
    pub fn new(post_keys: Vec<String>, last_post_score: Option<u64>) -> Self {
        Self {
            post_keys,
            last_post_score,
        }
    }

    // Iterate over tuples of (post_key, score) to extract the post keys and capture the last score
    pub fn from_scored_entries(entries: Vec<(String, f64)>) -> Self {
        let last_post_score = entries.last().map(|(_, score)| score.round() as u64);
        let post_keys = entries.into_iter().map(|(key, _)| key).collect();
        Self::new(post_keys, last_post_score)
    }

    pub fn is_empty(&self) -> bool {
        self.post_keys.is_empty()
    }
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Default)]
pub struct PostStream(pub Vec<PostView>);

impl RedisOps for PostStream {}

impl PostStream {
    pub fn extend(&mut self, post_stream: PostStream) {
        self.0.extend(post_stream.0);
    }
    pub async fn get_posts(
        source: StreamSource,
        pagination: Pagination,
        order: SortOrder,
        sorting: StreamSorting,
        viewer_id: Option<&str>,
        tags: Option<Vec<String>>,
        kind: Option<PubkyAppPostKind>,
    ) -> ModelResult<Option<Self>> {
        let post_key_stream =
            Self::collect_post_keys(source, pagination, order, sorting, tags, kind).await?;

        if post_key_stream.is_empty() {
            return Ok(None);
        }

        Self::from_listed_post_ids(viewer_id, &post_key_stream.post_keys).await
    }

    pub async fn get_post_keys(
        source: StreamSource,
        pagination: Pagination,
        order: SortOrder,
        sorting: StreamSorting,
        tags: Option<Vec<String>>,
        kind: Option<PubkyAppPostKind>,
    ) -> ModelResult<Option<PostKeyStream>> {
        let post_key_stream =
            Self::collect_post_keys(source, pagination, order, sorting, tags, kind).await?;

        if post_key_stream.is_empty() {
            return Ok(None);
        }

        Ok(Some(post_key_stream))
    }

    async fn collect_post_keys(
        source: StreamSource,
        pagination: Pagination,
        order: SortOrder,
        sorting: StreamSorting,
        tags: Option<Vec<String>>,
        kind: Option<PubkyAppPostKind>,
    ) -> ModelResult<PostKeyStream> {
        // Collection has its own envelope-driven resolution path (neither
        // sorted-set index nor Cypher).
        if let StreamSource::Collection { author_id, post_id } = &source {
            return Self::get_collection_items_post_keys(
                author_id,
                post_id,
                pagination.skip,
                pagination.limit,
            )
            .await;
        }

        // WoT sources emit observability metrics (spec v3.1). Capture the source
        // label and depth before `source` is consumed by the query below.
        let wot = match &source {
            StreamSource::Wot { depth, .. } => Some(("wot", depth.get())),
            // depth-0 is the "Me" self trust set; 1..=3 is the follow-network reach.
            StreamSource::WotDomain { trust, .. } => Some((
                "wot_domain",
                match trust {
                    DomainTrust::Me => 0,
                    DomainTrust::Network(depth) => depth.get(),
                },
            )),
            _ => None,
        };
        if let Some((source, depth)) = wot {
            super::metrics::record_wot_request(source, depth);
        }

        // Decide whether to use index or fallback to graph query
        let use_index = Self::can_use_index(&sorting, &source, &tags, &kind);

        let started = std::time::Instant::now();
        let result: ModelResult<PostKeyStream> = match use_index {
            true => Self::get_from_index(source, sorting, order, &tags, pagination).await,
            false => Self::get_from_graph(source, sorting, order, &tags, pagination, kind)
                .await
                .map_err(Into::into),
        };

        // Record duration on both success and error paths, so timeouts / DB errors
        // are not silently dropped from the latency histogram.
        if let Some((source, depth)) = wot {
            super::metrics::record_wot_result(
                source,
                depth,
                started.elapsed(),
                result.as_ref().ok().map(|keys| keys.post_keys.len()),
            );
        }

        result
    }

    // Determine if we have a quick access sorted set for this combination
    fn can_use_index(
        sorting: &StreamSorting,
        source: &StreamSource,
        tags: &Option<Vec<String>>,
        kind: &Option<PubkyAppPostKind>,
    ) -> bool {
        if kind.is_some() {
            return false;
        }
        match (sorting, source, tags) {
            // We have a sorted set for posts by a specific author
            (StreamSorting::Timeline, StreamSource::Author { .. }, None) => true,
            // We have a sorted set for global for any sorting
            (_, StreamSource::All, None) => true,
            // We have a sorted set for posts by tags for any sorting for a single tag
            (_, StreamSource::All, Some(tags)) if tags.len() == 1 => true,
            // We can use sorted set for posts by source only for timeline
            (StreamSorting::Timeline, StreamSource::Following { .. }, None) => true,
            (StreamSorting::Timeline, StreamSource::Followers { .. }, None) => true,
            (StreamSorting::Timeline, StreamSource::Friends { .. }, None) => true,
            // We have a sorted set for bookmarks only for timeline
            (StreamSorting::Timeline, StreamSource::Bookmarks { .. }, None) => true,
            // We can use sorted set of post replies
            (_, StreamSource::PostReplies { .. }, _) => true,
            // We can use sorted set of author replies
            (_, StreamSource::AuthorReplies { .. }, _) => true,
            // Other combinations require querying the graph
            _ => false,
        }
    }

    // Fetch posts from index
    async fn get_from_index(
        source: StreamSource,
        sorting: StreamSorting,
        order: SortOrder,
        tags: &Option<Vec<String>>,
        pagination: Pagination,
    ) -> ModelResult<PostKeyStream> {
        let start = pagination.start;
        let end = pagination.end;
        let skip = pagination.skip;
        let limit = pagination.limit;

        let result = match (source, tags) {
            // Global post streams
            (StreamSource::All, None) => {
                Self::get_global_posts_keys(sorting, order, start, end, skip, limit).await?
            }
            // Streams by tags
            (StreamSource::All, Some(tags)) if tags.len() == 1 => {
                Self::get_posts_keys_by_tag(&tags[0], sorting, start, end, skip, limit).await?
            }
            // Bookmark streams
            (StreamSource::Bookmarks { observer_id }, None) => {
                Self::get_bookmarked_posts(&observer_id, order, start, end, skip, limit).await?
            }
            // Stream of replies to specific a post
            (StreamSource::PostReplies { author_id, post_id }, None) => {
                Self::get_post_replies(&author_id, &post_id, order, start, end, skip, limit).await?
            }
            // Stream of parent post from a given author
            (StreamSource::Author { author_id }, None) => {
                Self::get_author_posts(&author_id, order, start, end, skip, limit, false).await?
            }
            // Streams of replies from a given author
            (StreamSource::AuthorReplies { author_id }, None) => {
                Self::get_author_posts(&author_id, order, start, end, skip, limit, true).await?
            }
            // Streams by simple source/reach: Following, Followers, Friends
            (source, None) => {
                Self::get_posts_by_source(source, order, start, end, skip, limit).await?
            }
            _ => PostKeyStream::default(),
        };
        Ok(result)
    }

    async fn get_collection_items_post_keys(
        author_id: &str,
        post_id: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> ModelResult<PostKeyStream> {
        let Some(details) = PostDetails::get_by_id(author_id, post_id).await? else {
            return Ok(PostKeyStream::default());
        };
        if !matches!(details.kind, PubkyAppPostKind::Collection) {
            return Ok(PostKeyStream::default());
        }
        let envelope: PubkyAppCollectionContent = match serde_json::from_str(&details.content) {
            Ok(env) => env,
            Err(e) => {
                warn!("Collection {author_id}:{post_id} envelope malformed: {e}");
                return Ok(PostKeyStream::default());
            }
        };

        let skip = skip.unwrap_or(0);
        let limit = limit.unwrap_or(usize::MAX);

        // Filter before slicing so dead refs don't shorten pages.
        let post_keys: Vec<String> = envelope
            .items
            .iter()
            .filter_map(|uri| match ParsedUri::try_from(uri.as_str()) {
                Ok(p) => match p.resource {
                    Resource::Post(item_post_id) => Some(format!("{}:{}", p.user_id, item_post_id)),
                    _ => None,
                },
                Err(_) => None,
            })
            .skip(skip)
            .take(limit)
            .collect();

        Ok(PostKeyStream::new(post_keys, None))
    }

    // Fetch posts from index
    async fn get_from_graph(
        source: StreamSource,
        sorting: StreamSorting,
        order: SortOrder,
        tags: &Option<Vec<String>>,
        pagination: Pagination,
        kind: Option<PubkyAppPostKind>,
    ) -> GraphResult<PostKeyStream> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::get::post_stream(source, sorting, order, tags, pagination, kind)?;

            // Set a 10-second timeout for the query execution
            result = match timeout(Duration::from_secs(10), graph.execute(query)).await {
                Ok(Ok(res)) => res, // Successfully executed within the timeout
                Ok(Err(e)) => return Err(GraphError::QueryFailed(e)), // Query failed
                Err(_) => return Err(GraphError::QueryTimeout), // Timeout error
            };
        }

        let mut post_keys = Vec::new();
        // Last row's sorting score (timestamp for timeline, engagement otherwise),
        // used as the pagination cursor.
        let mut last_post_score: Option<i64> = None;

        while let Some(row) = result.try_next().await? {
            let author_id: String = row.get("author_id")?;
            let post_id: String = row.get("post_id")?;
            let score: i64 = row.get("score")?;
            last_post_score = Some(score);
            post_keys.push(format!("{author_id}:{post_id}"));
        }

        Ok(PostKeyStream::new(
            post_keys,
            last_post_score.map(|s| s as u64),
        ))
    }

    pub async fn get_global_posts_keys(
        sorting: StreamSorting,
        order: SortOrder,
        start: Option<f64>,
        end: Option<f64>,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> RedisResult<PostKeyStream> {
        let sorted_set = match sorting {
            StreamSorting::TotalEngagement => {
                Self::try_from_index_sorted_set(
                    &POST_TOTAL_ENGAGEMENT_KEY_PARTS,
                    start,
                    end,
                    skip,
                    limit,
                    order,
                    None,
                )
                .await?
            }
            StreamSorting::Timeline => {
                Self::try_from_index_sorted_set(
                    &POST_TIMELINE_KEY_PARTS,
                    start,
                    end,
                    skip,
                    limit,
                    order,
                    None,
                )
                .await?
            }
        };
        Ok(PostKeyStream::from_scored_entries(
            sorted_set.unwrap_or_default(),
        ))
    }

    pub async fn get_posts_keys_by_tag(
        label: &str,
        sorting: StreamSorting,
        start: Option<f64>,
        end: Option<f64>,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> RedisResult<PostKeyStream> {
        let skip = skip.unwrap_or(0);
        let limit = limit.unwrap_or(10);

        let pag = Pagination {
            start,
            end,
            skip: Some(skip),
            limit: Some(limit),
        };

        let post_search_result = PostsByTagSearch::get_by_label(label, Some(sorting), pag).await?;

        let stream = match post_search_result {
            Some(post_keys) => {
                // Iterate over PostsByTagSearch structs to extract post keys and capture the last score
                let last_post_score = post_keys.last().map(|entry| entry.score as u64);
                let post_keys = post_keys
                    .into_iter()
                    .map(|post_score| post_score.post_key)
                    .collect();
                PostKeyStream::new(post_keys, last_post_score)
            }
            None => PostKeyStream::default(),
        };

        Ok(stream)
    }

    pub async fn get_author_posts(
        user_id: &str,
        order: SortOrder,
        start: Option<f64>,
        end: Option<f64>,
        skip: Option<usize>,
        limit: Option<usize>,
        replies: bool,
    ) -> RedisResult<PostKeyStream> {
        // Retrieve only parents or only reply posts written by the author from index
        let key_parts = match replies {
            true => POST_REPLIES_PER_USER_KEY_PARTS,
            false => POST_PER_USER_KEY_PARTS,
        };

        let key_parts = [&key_parts[..], &[user_id]].concat();
        let post_ids =
            Self::try_from_index_sorted_set(&key_parts, start, end, skip, limit, order, None)
                .await?;

        if let Some(post_ids) = post_ids {
            let post_keys = post_ids
                .into_iter()
                .map(|(post_id, score)| (format!("{user_id}:{post_id}"), score))
                .collect();
            Ok(PostKeyStream::from_scored_entries(post_keys))
        } else {
            Ok(PostKeyStream::default())
        }
    }

    pub async fn get_posts_by_source(
        source: StreamSource,
        order: SortOrder,
        start: Option<f64>,
        end: Option<f64>,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> ModelResult<PostKeyStream> {
        let custom_limit = Some(200);
        let mut user_ids = match &source {
            StreamSource::Following { observer_id } => {
                Following::get_by_id(observer_id, None, custom_limit)
                    .await?
                    .unwrap_or_default()
                    .0
            }
            StreamSource::Followers { observer_id } => {
                Followers::get_by_id(observer_id, None, custom_limit)
                    .await?
                    .unwrap_or_default()
                    .0
            }
            StreamSource::Friends { observer_id } => {
                Friends::get_by_id(observer_id, None, custom_limit)
                    .await?
                    .unwrap_or_default()
                    .0
            }
            _ => vec![],
        };

        if !user_ids.is_empty() {
            // Include the observer in the post stream
            if let Some(observer_id) = source.get_observer() {
                user_ids.push(observer_id.to_string());
            }

            let post_keys = Self::get_posts_for_user_ids(
                &user_ids.iter().map(AsRef::as_ref).collect::<Vec<_>>(),
                order,
                start,
                end,
                skip,
                limit,
            )
            .await?;
            Ok(PostKeyStream::from_scored_entries(post_keys))
        } else {
            Ok(PostKeyStream::default())
        }
    }

    pub async fn get_bookmarked_posts(
        user_id: &str,
        order: SortOrder,
        start: Option<f64>,
        end: Option<f64>,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> RedisResult<PostKeyStream> {
        let key_parts = [&BOOKMARKS_USER_KEY_PARTS[..], &[user_id]].concat();
        let post_keys =
            Self::try_from_index_sorted_set(&key_parts, start, end, skip, limit, order, None)
                .await?;

        Ok(PostKeyStream::from_scored_entries(
            post_keys.unwrap_or_default(),
        ))
    }

    pub async fn get_post_replies(
        author_id: &str,
        post_id: &str,
        order: SortOrder,
        start: Option<f64>,
        end: Option<f64>,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> RedisResult<PostKeyStream> {
        let key_parts = [&POST_REPLIES_PER_POST_KEY_PARTS[..], &[author_id, post_id]].concat();
        let post_replies =
            Self::try_from_index_sorted_set(&key_parts, start, end, skip, limit, order, None)
                .await?;
        Ok(PostKeyStream::from_scored_entries(
            post_replies.unwrap_or_default(),
        ))
    }

    // Streams for followers / followings / friends are expensive.
    // We are truncating to the first 200 user_ids. We could also random draw 200.
    // TODO rethink, we could also fallback to graph
    async fn get_posts_for_user_ids(
        user_ids: &[&str],
        order: SortOrder,
        start: Option<f64>,
        end: Option<f64>,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> ModelResult<Vec<(String, f64)>> {
        // Limit the number of user IDs to process to the first 200
        let max_user_ids = 200;
        let truncated_user_ids: Vec<String> = user_ids
            .iter()
            .take(max_user_ids)
            .map(|s| s.to_string())
            .collect();

        // Bounded to protect the pool; `buffered` keeps equal-score ties in input
        // order through the stable re-sort below; items owned so the future stays `Send`.
        let mut post_keys: Vec<(f64, String)> =
            stream::iter(truncated_user_ids.into_iter().map(|user_id| {
                let order = order.clone();
                async move {
                    let key_parts = [&POST_PER_USER_KEY_PARTS[..], &[user_id.as_str()]].concat();
                    let post_ids = Self::try_from_index_sorted_set(
                        &key_parts, start, end,
                        None, // We do not apply skip and limit here, as we need the full sorted set
                        None, order, None,
                    )
                    .await?;
                    Ok::<_, ModelError>(
                        post_ids
                            .map(|ids| {
                                ids.into_iter()
                                    .map(|(post_id, score)| (score, format!("{user_id}:{post_id}")))
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default(),
                    )
                }
            }))
            .buffered(8)
            .try_collect::<Vec<Vec<_>>>()
            .await?
            .into_iter()
            .flatten()
            .collect();

        // The selected user_ids does not have any post
        if post_keys.is_empty() {
            return Ok(Vec::new());
        }

        // Sort all the collected posts globally by their score (descending)
        post_keys.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Apply global skip and limit after sorting
        let start_index = skip.unwrap_or(0).clamp(0, post_keys.len());
        let end_index = limit
            .map(|l| (start_index + l).min(post_keys.len()))
            .unwrap_or(post_keys.len());

        // Ensure valid slice range
        if start_index >= end_index {
            return Ok(Vec::new());
        }

        let selected_post_keys = post_keys[start_index..end_index]
            .iter()
            .map(|(score, post_key)| (post_key.clone(), *score))
            .collect();

        Ok(selected_post_keys)
    }

    pub async fn from_listed_post_ids(
        viewer_id: Option<&str>,
        post_keys: &[String],
    ) -> ModelResult<Option<Self>> {
        let viewer_id = viewer_id.map(Arc::from);
        let mut handles = Vec::with_capacity(post_keys.len());

        for post_key in post_keys {
            let Some((author_id, post_id)) = post_key.split_once(':') else {
                warn!("Invalid post_key format (missing ':'): {post_key}");
                continue;
            };
            let author_id = author_id.to_string();
            let viewer_id = viewer_id.clone();
            let post_id = post_id.to_string();
            let handle = spawn(async move {
                PostView::get_by_id(&author_id, &post_id, viewer_id.as_deref(), None, None).await
            });
            handles.push(handle);
        }

        let mut post_views = Vec::with_capacity(post_keys.len());

        for handle in handles {
            if let Some(post_view) = handle.await.map_err(ModelError::from_generic)?? {
                post_views.push(post_view);
            }
        }

        Ok(Some(Self(post_views)))
    }

    /// Adds the post to a Redis sorted set using the `indexed_at` timestamp as the score.
    pub async fn add_to_timeline_sorted_set(details: &PostDetails) -> RedisResult<()> {
        let element = format!("{}:{}", details.author, details.id);
        let score = details.indexed_at as f64;
        Self::put_index_sorted_set(
            &POST_TIMELINE_KEY_PARTS,
            &[(score, element.as_str())],
            None,
            None,
        )
        .await
    }

    /// Adds the post to a Redis sorted set using the `indexed_at` timestamp as the score.
    pub async fn remove_from_timeline_sorted_set(
        author_id: &str,
        post_id: &str,
    ) -> RedisResult<()> {
        let element = format!("{author_id}:{post_id}");
        Self::remove_from_index_sorted_set(None, &POST_TIMELINE_KEY_PARTS, &[element.as_str()])
            .await
    }

    /// Adds the post to a Redis sorted set using the `indexed_at` timestamp as the score.
    pub async fn add_to_per_user_sorted_set(details: &PostDetails) -> RedisResult<()> {
        let key_parts = [&POST_PER_USER_KEY_PARTS[..], &[details.author.as_str()]].concat();
        let score = details.indexed_at as f64;
        Self::put_index_sorted_set(&key_parts, &[(score, details.id.as_str())], None, None).await
    }

    /// Adds the post to a Redis sorted set using the `indexed_at` timestamp as the score.
    pub async fn remove_from_per_user_sorted_set(
        author_id: &str,
        post_id: &str,
    ) -> RedisResult<()> {
        let key_parts = [&POST_PER_USER_KEY_PARTS[..], &[author_id]].concat();
        Self::remove_from_index_sorted_set(None, &key_parts, &[post_id]).await
    }

    /// Adds the post response to a Redis sorted set using the `indexed_at` timestamp as the score.
    pub async fn add_to_post_reply_sorted_set(
        // parent_user_id: &str,
        // parent_post_id: &str,
        parent_post_key_parts: &[&str; 2],
        author_id: &str,
        reply_id: &str,
        indexed_at: i64,
    ) -> RedisResult<()> {
        let key_parts = [&POST_REPLIES_PER_POST_KEY_PARTS[..], parent_post_key_parts].concat();
        let score = indexed_at as f64;
        let element = format!("{author_id}:{reply_id}");
        Self::put_index_sorted_set(&key_parts, &[(score, element.as_str())], None, None).await
    }

    /// Adds the post response to a Redis sorted set using the `indexed_at` timestamp as the score.
    pub async fn remove_from_post_reply_sorted_set(
        parent_post_key_parts: &[&str; 2],
        author_id: &str,
        reply_id: &str,
    ) -> RedisResult<()> {
        let key_parts = [&POST_REPLIES_PER_POST_KEY_PARTS[..], parent_post_key_parts].concat();
        let element = format!("{author_id}:{reply_id}");
        Self::remove_from_index_sorted_set(None, &key_parts, &[element.as_str()]).await
    }

    /// Adds the post to a Redis sorted set of replies per author using the `indexed_at` timestamp as the score.
    pub async fn add_to_replies_per_user_sorted_set(details: &PostDetails) -> RedisResult<()> {
        let key_parts = [
            &POST_REPLIES_PER_USER_KEY_PARTS[..],
            &[details.author.as_str()],
        ]
        .concat();
        let score = details.indexed_at as f64;
        Self::put_index_sorted_set(&key_parts, &[(score, details.id.as_str())], None, None).await
    }

    /// Adds the post to a Redis sorted set using the `indexed_at` timestamp as the score.
    pub async fn remove_from_replies_per_user_sorted_set(
        author_id: &str,
        post_id: &str,
    ) -> RedisResult<()> {
        let key_parts = [&POST_REPLIES_PER_USER_KEY_PARTS[..], &[author_id]].concat();
        Self::remove_from_index_sorted_set(None, &key_parts, &[post_id]).await
    }

    /// Adds a bookmark to Redis sorted set using the `indexed_at` timestamp as the score.
    pub async fn add_to_bookmarks_sorted_set(
        bookmark: &Bookmark,
        bookmarker_id: &str,
        post_id: &str,
        author_id: &str,
    ) -> RedisResult<()> {
        let key_parts = [&BOOKMARKS_USER_KEY_PARTS[..], &[bookmarker_id]].concat();
        let post_key = format!("{author_id}:{post_id}");
        let score = bookmark.indexed_at as f64;
        Self::put_index_sorted_set(&key_parts, &[(score, post_key.as_str())], None, None).await
    }

    /// Remove a bookmark from Redis sorted
    pub async fn remove_from_bookmarks_sorted_set(
        bookmarker_id: &str,
        post_id: &str,
        author_id: &str,
    ) -> RedisResult<()> {
        let key_parts = [&BOOKMARKS_USER_KEY_PARTS[..], &[bookmarker_id]].concat();
        let post_key = format!("{author_id}:{post_id}");
        Self::remove_from_index_sorted_set(None, &key_parts, &[&post_key]).await
    }

    /// Adds the post to a Redis sorted set using the total engagement as the score.
    pub async fn add_to_engagement_sorted_set(
        counts: &PostCounts,
        author_id: &str,
        post_id: &str,
    ) -> RedisResult<()> {
        let element = format!("{author_id}:{post_id}");
        let score = counts.tags + counts.replies + counts.reposts;
        let score = score as f64;

        Self::put_index_sorted_set(
            &POST_TOTAL_ENGAGEMENT_KEY_PARTS,
            &[(score, element.as_str())],
            None,
            None,
        )
        .await
    }

    pub async fn delete_from_engagement_sorted_set(
        author_id: &str,
        post_id: &str,
    ) -> RedisResult<()> {
        let post_key = format!("{author_id}:{post_id}");
        Self::remove_from_index_sorted_set(None, &POST_TOTAL_ENGAGEMENT_KEY_PARTS, &[&post_key])
            .await
    }

    pub async fn update_index_score(
        author_id: &str,
        post_id: &str,
        score_action: ScoreAction,
    ) -> RedisResult<()> {
        let post_key_slice = &[author_id, post_id];
        Self::put_score_index_sorted_set(
            &POST_TOTAL_ENGAGEMENT_KEY_PARTS,
            post_key_slice,
            score_action,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `can_use_index` short-circuits to the Cypher path whenever a kind filter
    /// is set, regardless of which kind — kind-filtered queries route via Cypher
    /// where the kind-specific filter actually applies.
    ///
    /// We parametrize across the source/sorting combinations that *would*
    /// otherwise be index-eligible (per the match arms below the early-return)
    /// to lock in that the kind short-circuit wins over the index path.
    #[test]
    fn test_can_use_index_returns_false_for_any_kind_filter() {
        let kinds_to_test = [
            PubkyAppPostKind::Short,
            PubkyAppPostKind::Long,
            PubkyAppPostKind::Image,
            PubkyAppPostKind::Video,
            PubkyAppPostKind::Link,
            PubkyAppPostKind::File,
            PubkyAppPostKind::Collection,
        ];

        // Combinations that would normally return `true` when kind is None.
        let index_eligible_combos = [
            (StreamSorting::Timeline, StreamSource::All),
            (StreamSorting::TotalEngagement, StreamSource::All),
            (
                StreamSorting::Timeline,
                StreamSource::Author {
                    author_id: "author".to_string(),
                },
            ),
            (
                StreamSorting::Timeline,
                StreamSource::Bookmarks {
                    observer_id: "observer".to_string(),
                },
            ),
        ];

        for kind in &kinds_to_test {
            for (sorting, source) in &index_eligible_combos {
                let result = PostStream::can_use_index(sorting, source, &None, &Some(kind.clone()));
                assert!(
                    !result,
                    "can_use_index({:?}, {:?}, None, Some({:?})) must return false",
                    sorting, source, kind
                );
            }
        }
    }

    /// Sanity counterpart: when no kind is set, `can_use_index` returns true
    /// for the index-eligible combinations. Locks in that the test above
    /// isn't passing because of a different bug elsewhere in the function.
    #[test]
    fn test_can_use_index_returns_true_for_no_kind_filter() {
        assert!(PostStream::can_use_index(
            &StreamSorting::Timeline,
            &StreamSource::All,
            &None,
            &None,
        ));
    }
}
