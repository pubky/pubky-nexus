use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::{queries, RedisOps};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents total counts of relationships of a user.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct PostCounts {
    pub tags: u32,
    pub replies: u32,
    pub reposts: u32,
}

impl RedisOps for PostCounts {}

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
        match Self::try_from_index(&[author_id, post_id], None).await? {
            Some(counts) => Ok(Some(counts)),
            None => Self::get_from_graph(author_id, post_id).await,
        }
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
            let counts = Self {
                tags: row.get("tags_count").unwrap_or_default(),
                replies: row.get("replies_count").unwrap_or_default(),
                reposts: row.get("reposts_count").unwrap_or_default(),
            };
            counts.set_index(&[author_id, post_id]).await?;
            Ok(Some(counts))
        } else {
            Ok(None)
        }
    }
}
