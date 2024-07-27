use std::error::Error;

use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::db::kv::index::{get_bool, set};
use crate::{queries, RedisOps};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::try_join;
use utoipa::ToSchema;

/// Represents the relationship of the user that views and user being viewed.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Relationship {
    pub following: bool,
    pub followed_by: bool,
}

/// This implementation reuses the "Follows:" model prefix for Redis keys, leveraging existing
/// data storage structures to minimize memory usage. Instead of storing separate `Relationship:`
/// objects, it stores boolean flags indicating the status of the relationship (i.e., whether
/// the user is followed by or following another user). This approach optimizes data storage
/// and retrieval in Redis, ensuring efficient memory utilization and streamlined operations.
///
/// The implementation provides methods for setting and retrieving relationship data using
/// these boolean flags, including handling potential errors and ensuring consistent key
/// structures for easy access and modification.
#[async_trait]
impl RedisOps for Relationship {
    async fn prefix() -> String {
        String::from("Follows")
    }

    async fn set_index(&self, key_parts: &[&str]) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (user_id, viewer_id) = match key_parts {
            [user_id, viewer_id] => (user_id, viewer_id),
            _ => return Err("Expected exactly two elements in key_parts".into()),
        };

        let prefix = Self::prefix().await;

        if self.followed_by {
            let key = format!("{user_id}:{viewer_id}");
            set(&prefix, &key, &true, None, None).await?;
        }
        if self.following {
            let key = format!("{viewer_id}:{user_id}");
            set(&prefix, &key, &true, None, None).await?;
        }

        Ok(())
    }

    async fn try_from_index(
        key_parts: &[&str],
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        let (user_id, viewer_id) = match key_parts {
            [user_id, viewer_id] => (user_id, viewer_id),
            _ => return Err("Expected exactly two elements in key_parts".into()),
        };

        let prefix = Self::prefix().await;

        let following_key = format!("{viewer_id}:{user_id}");
        let followed_by_key = format!("{user_id}:{viewer_id}");

        let (following_result, followed_by_result) = try_join!(
            get_bool(&prefix, &following_key),
            get_bool(&prefix, &followed_by_key)
        )?;

        if following_result.is_none() && followed_by_result.is_none() {
            return Ok(None);
        }

        Ok(Some(Self {
            following: following_result.unwrap_or(false),
            followed_by: followed_by_result.unwrap_or(false),
        }))
    }
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
