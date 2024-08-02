use crate::RedisOps;
use serde::{Deserialize, Serialize};
use std::error::Error;
use tokio::task::spawn;
use utoipa::ToSchema;

use super::{PostCounts, PostDetails, PostView};

#[derive(Deserialize, ToSchema)]
pub enum PostStreamSorting {
    Recency,
    TotalEngagement,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct PostStream(Vec<PostView>);

impl Default for PostStream {
    fn default() -> Self {
        Self::new()
    }
}

impl PostStream {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub async fn get_by_criteria(
        viewer_id: Option<String>,
        skip: Option<usize>,
        limit: Option<usize>,
        sorting: PostStreamSorting,
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        let post_ids = match sorting {
            PostStreamSorting::Recency => {
                Self::get_sorted_post_ids(skip, limit, "Posts:Timeline").await?
            }
            PostStreamSorting::TotalEngagement => {
                Self::get_sorted_post_ids(skip, limit, "Posts:TotalEngagement").await?
            }
        };

        match post_ids {
            Some(posts) => Self::from_listed_post_ids(&posts).await,
            None => Ok(None),
        }
    }

    pub async fn get_sorted_post_ids(
        skip: Option<usize>,
        limit: Option<usize>,
        key: &str,
    ) -> Result<Option<Vec<String>>, Box<dyn Error + Send + Sync>> {
        PostDetails::try_from_index_sorted_set(&[key], None, None, limit).await
    }

    pub async fn get_sorted_post_ids(
        skip: Option<usize>,
        limit: Option<usize>,
        key: &str,
    ) -> Result<Option<Vec<String>>, Box<dyn Error + Send + Sync>> {
        PostCounts::try_from_index_sorted_set(&[key], None, None, limit).await
    }

    pub async fn from_listed_post_ids(
        post_ids: &[String],
    ) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
        let mut handles = Vec::with_capacity(post_ids.len());

        for post_id in post_ids {
            let (author_id, post_id) = post_id.split_once(':').unwrap_or(("", ""));
            let author_id = author_id.to_string();
            let post_id = post_id.to_string();
            let handle =
                spawn(async move { PostView::get_by_id(&author_id, &post_id, None).await });
            handles.push(handle);
        }

        let mut post_views = Vec::with_capacity(post_ids.len());

        for handle in handles {
            if let Some(post_view) = handle.await?? {
                post_views.push(post_view);
            }
        }

        Ok(Some(Self(post_views)))
    }
}

impl PostDetails {
    /// Adds the post to a Redis sorted set using the `indexed_at` timestamp as the score.
    pub async fn add_to_recency_sorted_set(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key_parts = &["Posts", "Timeline"];
        let element = format!("{}:{}", self.author, self.id);
        let score = self.indexed_at as f64;

        Self::put_index_sorted_set(key_parts, &[(score, element.as_str())]).await
    }
}

impl PostCounts {
    /// Adds the post to a Redis sorted set using the total engagement as the score.
    pub async fn add_to_engagement_sorted_set(
        &self,
        author_id: &str,
        post_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key_parts = &["Posts", "TotalEngagement"];
        let element = format!("{}:{}", author_id, post_id);
        let score = self.tags + self.replies + self.reposts;
        let score = score as f64;

        Self::put_index_sorted_set(key_parts, &[(score, element.as_str())]).await
    }
}
