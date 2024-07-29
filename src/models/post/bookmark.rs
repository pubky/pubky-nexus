use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::{queries, RedisOps};
use chrono::Utc;
use neo4rs::Relation;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct Bookmark {
    id: String,
    indexed_at: i64,
}

impl RedisOps for Bookmark {}

impl Default for Bookmark {
    fn default() -> Self {
        Self::new()
    }
}

impl Bookmark {
    pub fn new() -> Self {
        Self {
            id: String::new(),
            indexed_at: Utc::now().timestamp(),
        }
    }

    /// Retrieves counts by user ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(
        author_id: &str,
        post_id: &str,
        viewer_id: Option<&str>,
    ) -> Result<Option<Bookmark>, Box<dyn std::error::Error + Send + Sync>> {
        // Return None early if no viewer_id supplied
        let viewer_id = match viewer_id {
            Some(viewer_id) => viewer_id,
            None => return Ok(None),
        };
        match Self::try_from_index(&[author_id, post_id, viewer_id]).await? {
            Some(counts) => Ok(Some(counts)),
            None => Self::get_from_graph(author_id, post_id, viewer_id).await,
        }
    }

    /// Retrieves the counts from Neo4j.
    pub async fn get_from_graph(
        author_id: &str,
        post_id: &str,
        viewer_id: &str,
    ) -> Result<Option<Bookmark>, Box<dyn std::error::Error + Send + Sync>> {
        let graph = get_neo4j_graph()?;
        let query = queries::post_bookmark(author_id, post_id, viewer_id);

        let graph = graph.lock().await;
        let mut result = graph.execute(query).await?;

        if let Some(row) = result.next().await? {
            let relation: Relation = row.get("b").unwrap();
            let counts = Self {
                id: relation.get("id").unwrap_or_default(),
                indexed_at: relation.get("indexed_at").unwrap_or_default(),
            };
            counts.set_index(&[author_id, post_id, viewer_id]).await?;
            Ok(Some(counts))
        } else {
            Ok(None)
        }
    }
}
