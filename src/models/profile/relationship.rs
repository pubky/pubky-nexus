use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::{queries, RedisOps};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents the relationship of the user that views and user being viewed.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Relationship {
    pub following: bool,
    pub followed_by: bool,
}

impl RedisOps for Relationship {}

impl Default for Relationship {
    fn default() -> Self {
        Self::new()
    }
}

impl Relationship {
    pub fn new() -> Self {
        Self {
            following: false,
            followed_by: false,
        }
    }

    // Retrieves user-viewer relationship
    pub async fn get_by_id(
        user_id: &str,
        viewer_id: Option<&str>,
    ) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
        match viewer_id {
            None => Ok(None),
            Some(v_id) => match Self::try_from_index(&[user_id, v_id]).await? {
                Some(indexed_relationship) => Ok(Some(indexed_relationship)),
                None => Self::get_from_graph(user_id, v_id).await,
            },
        }
    }

    /// Retrieves the relationship from Neo4j and indexes it in Redis.
    pub async fn get_from_graph(
        user_id: &str,
        viewer_id: &str,
    ) -> Result<Option<Relationship>, Box<dyn std::error::Error + Send + Sync>> {
        let graph = get_neo4j_graph()?;

        let query = queries::viewer_relationship(user_id, viewer_id);
        let graph = graph.lock().await;
        let mut result = graph.execute(query).await?;

        if let Some(row) = result.next().await? {
            let user_exists: bool = row.get("user_exists").unwrap_or(false);
            let viewer_exists: bool = row.get("viewer_exists").unwrap_or(false);

            if !user_exists || !viewer_exists {
                return Ok(None);
            }

            let relationship = Self {
                following: row.get("following").unwrap_or(false),
                followed_by: row.get("followed_by").unwrap_or(false),
            };
            relationship.set_index(&[user_id, viewer_id]).await?;
            Ok(Some(relationship))
        } else {
            Ok(None)
        }
    }
}
