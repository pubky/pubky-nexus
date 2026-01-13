use super::{Bookmark, PostCounts, PostDetails, PostView};
use crate::db::kv::{ScoreAction, SortOrder};
use crate::db::{get_neo4j_graph, queries, RedisOps};
use crate::models::{
    follow::{Followers, Following, Friends, UserFollows},
    post::search::PostsByTagSearch,
};
use crate::types::{DynError, Pagination, StreamSorting};
use pubky_app_specs::PubkyAppPostKind;
use serde::{Deserialize, Serialize};
use tokio::task::spawn;
use tokio::time::{timeout, Duration};
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
    #[default]
    All,
}

impl StreamSource {
    pub fn get_observer(&self) -> Option<&str> {
        match self {
            StreamSource::Followers { observer_id }
            | StreamSource::Following { observer_id }
            | StreamSource::Friends { observer_id }
            | StreamSource::Bookmarks { observer_id } => Some(observer_id),
            _ => None,
        }
    }

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
        viewer_id: Option<String>,
        tags: Option<Vec<String>>,
        kind: Option<PubkyAppPostKind>,
    ) -> Result<Option<Self>, DynError> {
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
    ) -> Result<Option<PostKeyStream>, DynError> {
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
    ) -> Result<PostKeyStream, DynError> {
        // Decide whether to use index or fallback to graph query
        let use_index = Self::can_use_index(&sorting, &source, &tags, &kind);

        let post_keys = match use_index {
            true => Self::get_from_index(source, sorting, order, &tags, pagination).await?,
            false => Self::get_from_graph(source, sorting, &tags, pagination, kind).await?,
        };

        Ok(post_keys)
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
    ) -> Result<PostKeyStream, DynError> {
        let start = pagination.start;
        let end = pagination.end;
        let skip = pagination.skip;
        let limit = pagination.limit;

        match (source, tags) {
            // Global post streams
            (StreamSource::All, None) => {
                Self::get_global_posts_keys(sorting, order, start, end, skip, limit).await
            }
            // Streams by tags
            (StreamSource::All, Some(tags)) if tags.len() == 1 => {
                Self::get_posts_keys_by_tag(&tags[0], sorting, start, end, skip, limit).await
            }
            // Bookmark streams
            (StreamSource::Bookmarks { observer_id }, None) => {
                Self::get_bookmarked_posts(&observer_id, order, start, end, skip, limit).await
            }
            // Stream of replies to specific a post
            (StreamSource::PostReplies { author_id, post_id }, None) => {
                Self::get_post_replies(&author_id, &post_id, order, start, end, skip, limit).await
            }
            // Stream of parent post from a given author
            (StreamSource::Author { author_id }, None) => {
                Self::get_author_posts(&author_id, order, start, end, skip, limit, false).await
            }
            // Streams of replies from a given author
            (StreamSource::AuthorReplies { author_id }, None) => {
                Self::get_author_posts(&author_id, order, start, end, skip, limit, true).await
            }
            // Streams by simple source/reach: Following, Followers, Friends
            (source, None) => {
                Self::get_posts_by_source(source, order, start, end, skip, limit).await
            }
            _ => Ok(PostKeyStream::default()),
        }
    }

    // Fetch posts from index
    async fn get_from_graph(
        source: StreamSource,
        sorting: StreamSorting,
        tags: &Option<Vec<String>>,
        pagination: Pagination,
        kind: Option<PubkyAppPostKind>,
    ) -> Result<PostKeyStream, DynError> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::get::post_stream(source, sorting, tags, pagination, kind);

            // Set a 10-second timeout for the query execution
            result = match timeout(Duration::from_secs(10), graph.execute(query)).await {
                Ok(Ok(res)) => res,                    // Successfully executed within the timeout
                Ok(Err(e)) => return Err(Box::new(e)), // Query failed
                Err(_) => return Err("Query timed out".into()), // Timeout error
            };
        }

        let mut post_keys = Vec::new();
        // Track the last post's indexed_at value
        let mut last_post_indexed_at: Option<i64> = None;

        while let Some(row) = result.next().await? {
            let author_id: String = row.get("author_id")?;
            let post_id: String = row.get("post_id")?;
            let indexed_at: i64 = row.get("indexed_at")?;
            let post_key = format!("{author_id}:{post_id}");
            // Track the last post's indexed_at by overwriting on each iteration
            last_post_indexed_at = Some(indexed_at);
            post_keys.push(post_key);
        }

        // Convert the last indexed_at to u64 for the score
        let last_post_score = last_post_indexed_at.map(|indexed_at| indexed_at as u64);

        Ok(PostKeyStream::new(post_keys, last_post_score))
    }

    pub async fn get_global_posts_keys(
        sorting: StreamSorting,
        order: SortOrder,
        start: Option<f64>,
        end: Option<f64>,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<PostKeyStream, DynError> {
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
    ) -> Result<PostKeyStream, DynError> {
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
    ) -> Result<PostKeyStream, DynError> {
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
    ) -> Result<PostKeyStream, DynError> {
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
    ) -> Result<PostKeyStream, DynError> {
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
    ) -> Result<PostKeyStream, DynError> {
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
    ) -> Result<Vec<(String, f64)>, DynError> {
        let mut post_keys = Vec::new();
        // Limit the number of user IDs to process to the first 200
        let max_user_ids = 200;
        let truncated_user_ids: Vec<&str> = user_ids.iter().take(max_user_ids).cloned().collect();

        // Retrieve posts for each user and collect them
        for user_id in &truncated_user_ids {
            let key_parts = [&POST_PER_USER_KEY_PARTS[..], &[user_id]].concat();
            if let Some(post_ids) = Self::try_from_index_sorted_set(
                &key_parts,
                start,
                end,
                None, // We do not apply skip and limit here, as we need the full sorted set
                None,
                order.clone(),
                None,
            )
            .await?
            {
                let user_post_keys: Vec<(f64, String)> = post_ids
                    .into_iter()
                    .map(|(post_id, score)| (score, format!("{user_id}:{post_id}")))
                    .collect();
                post_keys.extend(user_post_keys);
            }
        }

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
        viewer_id: Option<String>,
        post_keys: &[String],
    ) -> Result<Option<Self>, DynError> {
        let viewer_id = viewer_id.map(|id| id.to_string());
        let mut handles = Vec::with_capacity(post_keys.len());

        for post_key in post_keys {
            let (author_id, post_id) = post_key.split_once(':').unwrap_or_default();
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
            if let Some(post_view) = handle.await?? {
                post_views.push(post_view);
            }
        }

        Ok(Some(Self(post_views)))
    }

    /// Adds the post to a Redis sorted set using the `indexed_at` timestamp as the score.
    pub async fn add_to_timeline_sorted_set(details: &PostDetails) -> Result<(), DynError> {
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
    ) -> Result<(), DynError> {
        let element = format!("{author_id}:{post_id}");
        Self::remove_from_index_sorted_set(None, &POST_TIMELINE_KEY_PARTS, &[element.as_str()])
            .await
    }

    /// Adds the post to a Redis sorted set using the `indexed_at` timestamp as the score.
    pub async fn add_to_per_user_sorted_set(details: &PostDetails) -> Result<(), DynError> {
        let key_parts = [&POST_PER_USER_KEY_PARTS[..], &[details.author.as_str()]].concat();
        let score = details.indexed_at as f64;
        Self::put_index_sorted_set(&key_parts, &[(score, details.id.as_str())], None, None).await
    }

    /// Adds the post to a Redis sorted set using the `indexed_at` timestamp as the score.
    pub async fn remove_from_per_user_sorted_set(
        author_id: &str,
        post_id: &str,
    ) -> Result<(), DynError> {
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
    ) -> Result<(), DynError> {
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
    ) -> Result<(), DynError> {
        let key_parts = [&POST_REPLIES_PER_POST_KEY_PARTS[..], parent_post_key_parts].concat();
        let element = format!("{author_id}:{reply_id}");
        Self::remove_from_index_sorted_set(None, &key_parts, &[element.as_str()]).await
    }

    /// Adds the post to a Redis sorted set of replies per author using the `indexed_at` timestamp as the score.
    pub async fn add_to_replies_per_user_sorted_set(details: &PostDetails) -> Result<(), DynError> {
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
    ) -> Result<(), DynError> {
        let key_parts = [&POST_REPLIES_PER_USER_KEY_PARTS[..], &[author_id]].concat();
        Self::remove_from_index_sorted_set(None, &key_parts, &[post_id]).await
    }

    /// Adds a bookmark to Redis sorted set using the `indexed_at` timestamp as the score.
    pub async fn add_to_bookmarks_sorted_set(
        bookmark: &Bookmark,
        bookmarker_id: &str,
        post_id: &str,
        author_id: &str,
    ) -> Result<(), DynError> {
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
    ) -> Result<(), DynError> {
        let key_parts = [&BOOKMARKS_USER_KEY_PARTS[..], &[bookmarker_id]].concat();
        let post_key = format!("{author_id}:{post_id}");
        Self::remove_from_index_sorted_set(None, &key_parts, &[&post_key]).await
    }

    /// Adds the post to a Redis sorted set using the total engagement as the score.
    pub async fn add_to_engagement_sorted_set(
        counts: &PostCounts,
        author_id: &str,
        post_id: &str,
    ) -> Result<(), DynError> {
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
    ) -> Result<(), DynError> {
        let post_key = format!("{author_id}:{post_id}");
        Self::remove_from_index_sorted_set(None, &POST_TOTAL_ENGAGEMENT_KEY_PARTS, &[&post_key])
            .await
    }

    pub async fn update_index_score(
        author_id: &str,
        post_id: &str,
        score_action: ScoreAction,
    ) -> Result<(), DynError> {
        let post_key_slice = &[author_id, post_id];
        Self::put_score_index_sorted_set(
            &POST_TOTAL_ENGAGEMENT_KEY_PARTS,
            post_key_slice,
            score_action,
        )
        .await
    }
}
