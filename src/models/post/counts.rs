use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::{queries, RedisOps};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::PostStream;

/// Represents total counts of relationships of a user.
#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct PostCounts {
    pub tags: u32,
    pub replies: u32,
    pub reposts: u32,
}

impl RedisOps for PostCounts {}

impl PostCounts {
    /// Retrieves counts by user ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostCounts>, Box<dyn std::error::Error + Send + Sync>> {
        match Self::try_from_index_json(&[author_id, post_id]).await? {
            Some(counts) => Ok(Some(counts)),
            None => Self::get_from_graph(author_id, post_id).await,
        }
    }

    /// Retrieves the counts from Neo4j.
    pub async fn get_from_graph(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostCounts>, Box<dyn std::error::Error + Send + Sync>> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::read::post_counts(author_id, post_id);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        if let Some(row) = result.next().await? {
            if !row.get("post_exists").unwrap_or(false) {
                return Ok(None);
            }
            let counts = Self {
                tags: row.get("tags_count").unwrap_or_default(),
                replies: row.get("replies_count").unwrap_or_default(),
                reposts: row.get("reposts_count").unwrap_or_default(),
            };
            counts.put_index_json(&[author_id, post_id]).await?;
            PostStream::add_to_engagement_sorted_set(&counts, author_id, post_id).await?;
            Ok(Some(counts))
        } else {
            Ok(None)
        }
    }
}
