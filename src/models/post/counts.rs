use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::models::Prefix;
use crate::{index, queries};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents total counts of relationships of a profile.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct PostCounts {
    pub tags: u32,
    pub replies: u32,
    pub reposts: u32,
}

impl Default for PostCounts {
    fn default() -> Self {
        Self::new()
    }
}

impl PostCounts {
    pub fn new() -> Self {
        Self {
            tags: 0,
            replies: 0,
            reposts: 0,
        }
    }

    /// Retrieves counts by user ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostCounts>, Box<dyn std::error::Error + Send + Sync>> {
        match PostCounts::get_from_index(author_id, post_id).await? {
            Some(counts) => Ok(Some(counts)),
            None => PostCounts::get_from_graph(author_id, post_id).await,
        }
    }

    /// Sets counts in the Redis cache.
    pub async fn set_index(
        &self,
        author_id: &str,
        post_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key = format!("{}:{}", author_id, post_id);
        index::set(&Self::prefix(), &key, self, None, None).await
    }

    /// Get counts from the Redis cache.
    pub async fn get_from_index(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
        let key = format!("{}:{}", author_id, post_id);
        index::get(&Self::prefix(), &key, None).await
    }

    /// Retrieves the counts from Neo4j.
    pub async fn get_from_graph(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostCounts>, Box<dyn std::error::Error + Send + Sync>> {
        let graph = get_neo4j_graph()?;
        let query = queries::post_counts(author_id, post_id);

        let graph = graph.lock().await;
        let mut result = graph.execute(query).await?;

        if let Some(row) = result.next().await? {
            if !row.get("post_exists").unwrap_or(false) {
                return Ok(None);
            }
            let counts = PostCounts {
                tags: row.get("tags_count").unwrap_or_default(),
                replies: row.get("replies_count").unwrap_or_default(),
                reposts: row.get("reposts_count").unwrap_or_default(),
            };
            counts.set_index(author_id, post_id).await?;
            Ok(Some(counts))
        } else {
            Ok(None)
        }
    }
}
