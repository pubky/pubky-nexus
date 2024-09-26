use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::db::kv::index::json::JsonAction;
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
        match Self::get_from_index(author_id, post_id).await? {
            Some(counts) => Ok(Some(counts)),
            None => {
                let graph_response = Self::get_from_graph(author_id, post_id).await?;
                if let Some(post_counts) = graph_response {
                    post_counts.put_to_index(author_id, post_id).await?;
                    return Ok(Some(post_counts));
                }
                Ok(None)
            }
        }
    }

    pub async fn get_from_index(
        author_id: &str,
        post_id: &str,
    ) -> Result<Option<PostCounts>, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(post_counts) = Self::try_from_index_json(&[author_id, post_id]).await? {
            return Ok(Some(post_counts));
        }
        Ok(None)
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
            let post_exists: bool = row.get("exists").unwrap_or(false);
            if post_exists {
                match row.get("counts") {
                    Ok(post_counts) => return Ok(Some(post_counts)),
                    Err(_e) => return Ok(None),
                }
            }
        }
        Ok(None)
    }

    pub async fn put_to_index(
        &self,
        author_id: &str,
        post_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.put_index_json(&[author_id, post_id]).await?;
        PostStream::add_to_engagement_sorted_set(self, author_id, post_id).await?;
        Ok(())
    }

    pub async fn update_index_field(
        index_key: &[&str],
        field: &str,
        action: JsonAction,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Self::modify_json_field(index_key, field, action).await?;
        Ok(())
    }

    pub async fn reindex(
        author_id: &str,
        post_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match Self::get_from_graph(author_id, post_id).await? {
            Some(counts) => counts.put_to_index(author_id, post_id).await?,
            None => log::error!(
                "{}:{} Could not found post counts in the graph",
                author_id,
                post_id
            ),
        }
        Ok(())
    }
}
