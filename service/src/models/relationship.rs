use pk_social_common::connectors::{
    neo4j::get_neo4j_graph,
    redis::{get_redis_conn, AsyncCommands},
};
use pk_social_common::queries;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const RELATIONSHIP_PREFIX: &str = "follows!";

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

    /// Retrieves a relationship by user ids, checking the cache first and then the graph database.
    pub async fn get(
        user_id: &str,
        viewer_id: &str,
    ) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        // Try to get from indexed cache
        if let Some(indexed_relationship) = Self::get_from_index(user_id, viewer_id).await? {
            // println!("Found relationship index {user_id}&{viewer_id}");
            return Ok(Some(indexed_relationship));
        }

        // Fallback to query from graph
        Self::get_from_graph(user_id, viewer_id).await
    }

    /// Indexes the relationship in Redis.
    pub async fn set_index(
        user_id: &str,
        viewer_id: &str,
        relationship: &Relationship,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut redis_conn = get_redis_conn().await?;

        if relationship.followed_by {
            let key = format!("{RELATIONSHIP_PREFIX}{user_id}{viewer_id}");
            // println!("Saved relationship  index {key}");
            redis_conn.set_ex(key, true, 3600).await?;
        }
        if relationship.following {
            let key = format!("{RELATIONSHIP_PREFIX}{viewer_id}{user_id}");
            redis_conn.set_ex(key, true, 3600).await?;
        }

        Ok(())
    }

    /// Retrieves the relationship from Redis.
    pub async fn get_from_index(
        user_id: &str,
        viewer_id: &str,
    ) -> Result<Option<Relationship>, Box<dyn std::error::Error>> {
        let mut redis_conn = get_redis_conn().await?;

        let following_key = format!("{RELATIONSHIP_PREFIX}{viewer_id}{user_id}");
        let followed_by_key = format!("{RELATIONSHIP_PREFIX}{user_id}{viewer_id}");

        let following_result: Option<bool> = redis_conn.get(following_key).await?;
        let followed_by_result: Option<bool> = redis_conn.get(followed_by_key).await?;

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
    ) -> Result<Option<Relationship>, Box<dyn std::error::Error>> {
        let graph = get_neo4j_graph()?;

        let viewer_relationship_query = queries::viewer_relationship(user_id, viewer_id);
        let graph = graph.lock().await;
        let mut result = graph.execute(viewer_relationship_query).await?;

        if let Some(row) = result.next().await? {
            let relationship = Relationship {
                following: row.get("following").unwrap_or(false),
                followed_by: row.get("followed_by").unwrap_or(false),
            };
            Relationship::set_index(user_id, viewer_id, &relationship).await?;
            Ok(Some(relationship))
        } else {
            Ok(None)
        }
    }
}
