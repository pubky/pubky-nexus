use super::{Bookmark, PostCounts, PostDetails, PostView};
use crate::{
    db::kv::index::sorted_sets::Sorting,
    get_neo4j_graph,
    models::{
        tag::search::TagSearch,
        user::{Followers, Following, Friends, UserFollows},
    },
    queries, RedisOps, ScoreAction,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use tokio::task::spawn;
use utoipa::ToSchema;

pub const POST_TIMELINE_KEY_PARTS: [&str; 3] = ["Posts", "Global", "Timeline"];
pub const POST_TOTAL_ENGAGEMENT_KEY_PARTS: [&str; 3] = ["Posts", "Global", "TotalEngagement"];
pub const POST_PER_USER_KEY_PARTS: [&str; 2] = ["Posts", "User"];
const BOOKMARKS_USER_KEY_PARTS: [&str; 2] = ["Bookmarks", "User"];

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum PostStreamSorting {
    Timeline,
    TotalEngagement,
}

#[derive(Deserialize, ToSchema, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PostStreamReach {
    All,
    Following,
    Followers,
    Friends,
    Bookmarks,
}

#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct PostStream(pub Vec<PostView>);

impl RedisOps for PostStream {}

impl Default for PostStream {
    fn default() -> Self {
        Self::new()
    }
}

impl PostStream {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub async fn get_posts(
        viewer_id: Option<String>,
        author_id: Option<String>,
        sorting: PostStreamSorting,
        reach: PostStreamReach,
        tag: Option<String>,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        // Decide whether to use index or fallback to graph query
        let use_index = Self::can_use_index(&sorting, &author_id, &reach, &tag);

        let post_keys = match use_index {
            true => {
                Self::get_from_index(
                    viewer_id.clone(),
                    author_id,
                    sorting,
                    reach,
                    tag,
                    skip,
                    limit,
                )
                .await?
            }
            false => {
                Self::get_from_graph(
                    viewer_id.clone(),
                    author_id,
                    sorting,
                    reach,
                    tag,
                    skip,
                    limit,
                )
                .await?
            }
        };

        if post_keys.is_empty() {
            return Ok(None);
        }

        Self::from_listed_post_ids(viewer_id, &post_keys).await
    }

    // Determine if we have a quick access sorted set for this combination
    fn can_use_index(
        sorting: &PostStreamSorting,
        author_id: &Option<String>,
        reach: &PostStreamReach,
        tags: &Option<String>,
    ) -> bool {
        match (sorting, reach, tags, author_id) {
            // We have a sorted set for posts by a specific author
            (PostStreamSorting::Timeline, _, None, Some(_)) => true,
            // We have a sorted set for global for any sorting
            (_, PostStreamReach::All, None, None) => true,
            // We have a sorted set for posts by tag for any sorting
            (_, PostStreamReach::All, Some(_), _) => true,
            // We can use sorted set for posts by reach only for timeline
            (PostStreamSorting::Timeline, PostStreamReach::Following, None, None) => true,
            (PostStreamSorting::Timeline, PostStreamReach::Followers, None, None) => true,
            (PostStreamSorting::Timeline, PostStreamReach::Friends, None, None) => true,
            // We have a sorted set for bookmarks only for timeline
            (PostStreamSorting::Timeline, PostStreamReach::Bookmarks, None, None) => true,
            // Other combinations require querying the graph
            _ => false,
        }
    }

    // Fetch posts from index
    async fn get_from_index(
        viewer_id: Option<String>,
        author_id: Option<String>,
        sorting: PostStreamSorting,
        reach: PostStreamReach,
        tag: Option<String>,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        match (reach, tag, author_id) {
            // Global post streams
            (PostStreamReach::All, None, None) => {
                Self::get_global_posts_keys(sorting, skip, limit).await
            }
            // Streams by tag
            (PostStreamReach::All, Some(label), None) => {
                Self::get_posts_keys_by_tag(&label, sorting, skip, limit).await
            }
            // Bookmark streams
            (PostStreamReach::Bookmarks, None, None) => {
                Self::get_bookmarked_posts(
                    &viewer_id.ok_or("Viewer ID is required for bookmark streams")?,
                    skip,
                    limit,
                )
                .await
            }
            // Streams by simple reach
            (reach, None, None) => Self::get_posts_by_reach(reach, viewer_id, skip, limit).await,
            // Streams by only author
            (_, None, Some(author_id)) => Self::get_user_posts(&author_id, skip, limit).await,
            _ => Ok(vec![]),
        }
    }

    // Fetch posts from index
    async fn get_from_graph(
        viewer_id: Option<String>,
        author_id: Option<String>,
        sorting: PostStreamSorting,
        reach: PostStreamReach,
        tags: Option<String>,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query =
                queries::get::post_stream(viewer_id, author_id, reach, tags, sorting, skip, limit);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        let mut post_keys = Vec::new();

        while let Some(row) = result.next().await? {
            let author_id: String = row.get("author_id")?;
            let post_id: String = row.get("post_id")?;
            post_keys.push(format!("{}:{}", author_id, post_id));
        }

        Ok(post_keys)
    }

    pub async fn get_global_posts_keys(
        sorting: PostStreamSorting,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        let sorted_set = match sorting {
            PostStreamSorting::TotalEngagement => {
                Self::try_from_index_sorted_set(
                    &POST_TOTAL_ENGAGEMENT_KEY_PARTS,
                    None,
                    None,
                    skip,
                    limit,
                    Sorting::Descending,
                )
                .await?
            }
            PostStreamSorting::Timeline => {
                Self::try_from_index_sorted_set(
                    &POST_TIMELINE_KEY_PARTS,
                    None,
                    None,
                    skip,
                    limit,
                    Sorting::Descending,
                )
                .await?
            }
        };
        match sorted_set {
            Some(post_keys) => Ok(post_keys.into_iter().map(|(key, _)| key).collect()),
            None => Ok(vec![]),
        }
    }

    pub async fn get_posts_keys_by_tag(
        label: &str,
        sorting: PostStreamSorting,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        let skip = skip.unwrap_or(0);
        let limit = limit.unwrap_or(10);

        let post_search_result = TagSearch::get_by_label(label, Some(sorting), skip, limit).await?;

        match post_search_result {
            Some(post_keys) => Ok(post_keys
                .into_iter()
                .map(|post_score| post_score.post_key)
                .collect()),
            None => Ok(vec![]),
        }
    }

    pub async fn get_user_posts(
        user_id: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        let key_parts = [&POST_PER_USER_KEY_PARTS[..], &[user_id]].concat();
        let post_ids = Self::try_from_index_sorted_set(
            &key_parts,
            None,
            None,
            skip,
            limit,
            Sorting::Descending,
        )
        .await?;

        if let Some(post_ids) = post_ids {
            let post_keys = post_ids
                .into_iter()
                .map(|(post_id, _)| format!("{}:{}", user_id, post_id))
                .collect();
            Ok(post_keys)
        } else {
            Ok(vec![])
        }
    }

    pub async fn get_posts_by_reach(
        reach: PostStreamReach,
        viewer_id: Option<String>,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        let viewer_id = match viewer_id {
            None => return Ok(vec![]),
            Some(v_id) => v_id,
        };

        let user_ids = match reach {
            PostStreamReach::Following => {
                Following::get_by_id(&viewer_id, None, None)
                    .await?
                    .unwrap_or_default()
                    .0
            }
            PostStreamReach::Followers => {
                Followers::get_by_id(&viewer_id, None, None)
                    .await?
                    .unwrap_or_default()
                    .0
            }
            PostStreamReach::Friends => {
                Friends::get_by_id(&viewer_id, None, None)
                    .await?
                    .unwrap_or_default()
                    .0
            }
            _ => vec![],
        };

        if !user_ids.is_empty() {
            let post_keys = Self::get_posts_for_user_ids(
                &user_ids.iter().map(AsRef::as_ref).collect::<Vec<_>>(),
                skip,
                limit,
            )
            .await?;
            Ok(post_keys)
        } else {
            Ok(vec![])
        }
    }

    pub async fn get_bookmarked_posts(
        user_id: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        let key_parts = [&BOOKMARKS_USER_KEY_PARTS[..], &[user_id]].concat();
        let post_keys = Self::try_from_index_sorted_set(
            &key_parts,
            None,
            None,
            skip,
            limit,
            Sorting::Descending,
        )
        .await?;

        if let Some(post_keys) = post_keys {
            Ok(post_keys.into_iter().map(|(key, _)| key).collect())
        } else {
            Ok(vec![])
        }
    }

    // Streams for followers / followings / friends are expensive.
    // We are truncating to the first 200 user_ids. We could also random draw 200.
    // TODO rethink, we could also fallback to graph
    async fn get_posts_for_user_ids(
        user_ids: &[&str],
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        let mut post_keys = Vec::new();

        // Limit the number of user IDs to process to the first 200
        let max_user_ids = 200;
        let truncated_user_ids: Vec<&str> = user_ids.iter().take(max_user_ids).cloned().collect();

        // Retrieve posts for each user and collect them
        for user_id in &truncated_user_ids {
            let key_parts = [&POST_PER_USER_KEY_PARTS[..], &[user_id]].concat();
            if let Some(post_ids) = Self::try_from_index_sorted_set(
                &key_parts,
                None,
                None,
                None, // We do not apply skip and limit here, as we need the full sorted set
                None,
                Sorting::Descending,
            )
            .await?
            {
                let user_post_keys: Vec<(f64, String)> = post_ids
                    .into_iter()
                    .map(|(post_id, score)| (score, format!("{}:{}", user_id, post_id)))
                    .collect();
                post_keys.extend(user_post_keys);
            }
        }

        // Sort all the collected posts globally by their score (descending)
        post_keys.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Apply global skip and limit after sorting
        let start_index = skip.unwrap_or(0).max(0);
        let end_index = if let Some(limit) = limit {
            (start_index + limit).min(post_keys.len())
        } else {
            post_keys.len()
        };

        let selected_post_keys = post_keys[start_index..end_index]
            .iter()
            .map(|(_, post_key)| post_key.clone())
            .collect();

        Ok(selected_post_keys)
    }

    pub async fn from_listed_post_ids(
        viewer_id: Option<String>,
        post_keys: &[String],
    ) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
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
    pub async fn add_to_timeline_sorted_set(
        details: &PostDetails,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let element = format!("{}:{}", details.author, details.id);
        let score = details.indexed_at as f64;
        Self::put_index_sorted_set(&POST_TIMELINE_KEY_PARTS, &[(score, element.as_str())]).await
    }

    /// Adds the post to a Redis sorted set using the `indexed_at` timestamp as the score.
    pub async fn add_to_per_user_sorted_set(
        details: &PostDetails,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key_parts = [&POST_PER_USER_KEY_PARTS[..], &[details.author.as_str()]].concat();
        let score = details.indexed_at as f64;
        Self::put_index_sorted_set(&key_parts, &[(score, details.id.as_str())]).await
    }

    /// Adds a bookmark to Redis sorted set using the `indexed_at` timestamp as the score.
    pub async fn add_to_bookmarks_sorted_set(
        bookmark: &Bookmark,
        bookmarker_id: &str,
        post_id: &str,
        author_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key_parts = [&BOOKMARKS_USER_KEY_PARTS[..], &[bookmarker_id]].concat();
        let post_key = format!("{}:{}", author_id, post_id);
        let score = bookmark.indexed_at as f64;
        Self::put_index_sorted_set(&key_parts, &[(score, post_key.as_str())]).await
    }

    /// Remove a bookmark from Redis sorted
    pub async fn remove_from_bookmarks_sorted_set(
        bookmarker_id: &str,
        post_id: &str,
        author_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key_parts = [&BOOKMARKS_USER_KEY_PARTS[..], &[bookmarker_id]].concat();
        let post_key = format!("{}:{}", author_id, post_id);
        Self::remove_from_index_sorted_set(&key_parts, &[&post_key]).await
    }

    /// Adds the post to a Redis sorted set using the total engagement as the score.
    pub async fn add_to_engagement_sorted_set(
        counts: &PostCounts,
        author_id: &str,
        post_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let element = format!("{}:{}", author_id, post_id);
        let score = counts.tags + counts.replies + counts.reposts;
        let score = score as f64;

        Self::put_index_sorted_set(
            &POST_TOTAL_ENGAGEMENT_KEY_PARTS,
            &[(score, element.as_str())],
        )
        .await
    }

    pub async fn update_index_score(
        author_id: &str,
        post_id: &str,
        score_action: ScoreAction,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let post_key_slice = &[author_id, post_id];
        Self::put_score_index_sorted_set(
            &POST_TOTAL_ENGAGEMENT_KEY_PARTS,
            post_key_slice,
            score_action,
        )
        .await
    }
}
