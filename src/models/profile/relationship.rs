use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::{index, prefix, queries};
use serde::{Deserialize, Serialize};
use tokio::join;
use utoipa::ToSchema;

/// Represents the relationship of the user that views and user being viewed.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Relationship {
    pub following: bool,
    pub followed_by: bool,
}

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
            Some(v_id) => match Self::get_from_index(user_id, v_id).await? {
                Some(indexed_relationship) => Ok(Some(indexed_relationship)),
                None => Self::get_from_graph(user_id, v_id).await,
            },
        }
    }

    /// Indexes the relationship in Redis.
    pub async fn set_index(
        &self,
        user_id: &str,
        viewer_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.followed_by {
            let key = format!("{user_id}{viewer_id}");
            index::set_bool(prefix::RELATIONSHIP, &key, true, None).await?;
        }
        if self.following {
            let key = format!("{viewer_id}{user_id}");
            index::set_bool(prefix::RELATIONSHIP, &key, true, None).await?;
        }

        Ok(())
    }

    /// Retrieves the relationship from Redis.
    pub async fn get_from_index(
        user_id: &str,
        viewer_id: &str,
    ) -> Result<Option<Relationship>, Box<dyn std::error::Error + Send + Sync>> {
        let following_key = format!("{viewer_id}{user_id}");
        let followed_by_key = format!("{user_id}{viewer_id}");

        // Perform both operations concurrently
        let (following_result, followed_by_result) = join!(
            index::get_bool(prefix::RELATIONSHIP, &following_key),
            index::get_bool(prefix::RELATIONSHIP, &followed_by_key),
        );

        // Unwrap the results
        let following_result: Option<bool> = following_result?;
        let followed_by_result: Option<bool> = followed_by_result?;

        match (following_result, followed_by_result) {
            (Some(following), Some(followed_by)) => Ok(Some(Relationship {
                following,
                followed_by,
            })),
            (None, None) => Ok(None),
            (Some(following), None) => Ok(Some(Relationship {
                following,
                followed_by: false,
            })),
            (None, Some(followed_by)) => Ok(Some(Relationship {
                following: false,
                followed_by,
            })),
        }
    }

    /// Retrieves the relationship from Neo4j and indexes it in Redis.
    pub async fn get_from_graph(
        user_id: &str,
        viewer_id: &str,
    ) -> Result<Option<Relationship>, Box<dyn std::error::Error + Send + Sync>> {
        let graph = get_neo4j_graph()?;

        let viewer_relationship_query = queries::viewer_relationship(user_id, viewer_id);
        let graph = graph.lock().await;
        let mut result = graph.execute(viewer_relationship_query).await?;

        if let Some(row) = result.next().await? {
            let user_exists: bool = row.get("user_exists").unwrap_or(false);
            let viewer_exists: bool = row.get("viewer_exists").unwrap_or(false);

            if !user_exists || !viewer_exists {
                return Ok(None);
            }

            let relationship = Relationship {
                following: row.get("following").unwrap_or(false),
                followed_by: row.get("followed_by").unwrap_or(false),
            };
            relationship.set_index(user_id, viewer_id).await?;
            Ok(Some(relationship))
        } else {
            Ok(None)
        }
    }
}
